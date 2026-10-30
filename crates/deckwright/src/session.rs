//! Persistent query sessions behind `dw serve`. Protocol: docs/decisions/session.md.
//!
//! Each open handle owns one package snapshot. The query session and the check
//! package are built from the same bytes, so they always describe one revision.
use crate::{Error, Limits, Package, Result, check, query::QuerySession, xml};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// File metadata used for staleness. Compared on every query, never cached per op.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Meta {
    len: u64,
    modified: Option<SystemTime>,
}

impl Meta {
    fn of(metadata: &fs::Metadata) -> Self {
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }
}

struct Entry {
    path: PathBuf,
    meta: Meta,
    package: Package,
    session: QuerySession,
    revision: u32,
    last_used: u64,
}

impl Entry {
    fn is_fresh(&self) -> bool {
        fs::metadata(&self.path).is_ok_and(|m| Meta::of(&m) == self.meta)
    }
}

struct Loaded {
    meta: Meta,
    package: Package,
    session: QuerySession,
    slides: usize,
}

struct Fail {
    code: &'static str,
    message: String,
}

fn fail(code: &'static str, message: impl Into<String>) -> Fail {
    Fail {
        code,
        message: message.into(),
    }
}

fn io_error(error: Error) -> Fail {
    fail("io", error.to_string())
}

fn query_error(error: Error) -> Fail {
    fail("query", error.to_string())
}

/// Reads the file once and builds both the package and the query session from those bytes.
fn load(path: &Path) -> std::result::Result<Loaded, Fail> {
    let limits = Limits::default();
    let io = |source| {
        io_error(Error::Io {
            path: path.to_path_buf(),
            source,
        })
    };
    let file = fs::File::open(path).map_err(io)?;
    // Metadata is taken before the read: a concurrent rewrite then reads as stale later.
    let meta = Meta::of(&file.metadata().map_err(io)?);
    let mut bytes = Vec::new();
    file.take(limits.max_input_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io)?;
    let package = Package::from_bytes(bytes, limits).map_err(io_error)?;
    let session = QuerySession::from_package(&package, limits).map_err(io_error)?;
    let presentation = xml::parse(
        package.part("ppt/presentation.xml").unwrap_or_default(),
        "ppt/presentation.xml",
        limits.max_xml_depth,
    )
    .map_err(io_error)?;
    let slides = presentation.descendants("sldId").count();
    Ok(Loaded {
        meta,
        package,
        session,
        slides,
    })
}

fn text<'a>(request: &'a Map<String, Value>, name: &str) -> std::result::Result<&'a str, Fail> {
    request
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| fail("parse", format!("{name} must be a string")))
}

fn number(request: &Map<String, Value>, name: &str) -> std::result::Result<u64, Fail> {
    request
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| fail("parse", format!("{name} must be a non-negative integer")))
}

/// Out-of-range indexes are left for QuerySession to reject as `query`.
fn index(request: &Map<String, Value>, name: &str) -> std::result::Result<usize, Fail> {
    Ok(usize::try_from(number(request, name)?).unwrap_or(usize::MAX))
}

fn clicks(request: &Map<String, Value>) -> std::result::Result<Vec<u64>, Fail> {
    match request.get("clicks_ms") {
        None => Ok(vec![]),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_u64()
                    .ok_or_else(|| fail("parse", "clicks_ms must hold non-negative integers"))
            })
            .collect(),
        Some(_) => Err(fail("parse", "clicks_ms must be an array")),
    }
}

/// Handles that were dropped by the cache report `evicted`; all others are `invalid-handle`.
fn missing(evicted: &HashSet<String>, handle: &str) -> Fail {
    if evicted.contains(handle) {
        fail(
            "evicted",
            format!("handle {handle} was evicted from the cache"),
        )
    } else {
        fail("invalid-handle", format!("handle {handle} is not open"))
    }
}

fn serialize<T: Serialize>(report: T) -> Value {
    // Report types contain only strings, integers and finite-or-null floats.
    serde_json::to_value(report).expect("query and check reports serialize")
}

/// Reports carry their own `revision`; overwrite it with the session's current revision.
fn query_report<T: Serialize>(
    result: Result<T>,
    revision: u32,
) -> std::result::Result<Value, Fail> {
    let mut value = serialize(result.map_err(query_error)?);
    if let Some(object) = value.as_object_mut() {
        object.insert("revision".into(), json!(revision));
    }
    Ok(value)
}

/// Protocol state for `dw serve`: a bounded LRU set of open package snapshots.
pub struct Server {
    max_open: usize,
    clock: u64,
    next_handle: u64,
    entries: HashMap<String, Entry>,
    evicted: HashSet<String>,
}

impl Server {
    /// `max_open` is clamped to at least 1.
    pub fn new(max_open: usize) -> Self {
        Self {
            max_open: max_open.max(1),
            clock: 0,
            next_handle: 1,
            entries: HashMap::new(),
            evicted: HashSet::new(),
        }
    }

    /// Answers one request line with one JSON response line (no trailing newline).
    pub fn handle_line(&mut self, line: &str) -> String {
        let (id, outcome) = match serde_json::from_str::<Value>(line) {
            Ok(Value::Object(request)) => {
                let id = request.get("id").cloned().unwrap_or(Value::Null);
                (id, self.dispatch(&request))
            }
            Ok(_) => (
                Value::Null,
                Err(fail("parse", "request must be a JSON object")),
            ),
            Err(error) => (
                Value::Null,
                Err(fail("parse", format!("invalid JSON: {error}"))),
            ),
        };
        let envelope = match outcome {
            Ok(result) => json!({"id": id, "ok": true, "result": result}),
            Err(f) => json!({
                "id": id,
                "ok": false,
                "error": {"code": f.code, "message": f.message},
            }),
        };
        envelope.to_string()
    }

    fn dispatch(&mut self, request: &Map<String, Value>) -> std::result::Result<Value, Fail> {
        let op = request
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| fail("parse", "op must be a string"))?;
        match op {
            "open" => self.open(request),
            "refresh" => self.refresh(request),
            "close" => self.close(request),
            "state" => {
                let slide = index(request, "slide")?;
                let at = number(request, "at_ms")?;
                let clicks = clicks(request)?;
                self.report_op(request, |e| {
                    query_report(e.session.state(slide, &clicks, at), e.revision)
                })
            }
            "timeline" => {
                let slide = index(request, "slide")?;
                self.report_op(request, |e| {
                    query_report(e.session.timeline(slide), e.revision)
                })
            }
            "morph" => {
                let from = index(request, "from")?;
                let to = index(request, "to")?;
                self.report_op(request, |e| {
                    query_report(e.session.morph(from, to), e.revision)
                })
            }
            "check" => self.report_op(request, |e| Ok(serialize(check::check_package(&e.package)))),
            other => Err(fail("unknown-op", format!("unknown op {other}"))),
        }
    }

    /// Looks up a handle, refuses stale snapshots, runs `run`, and counts a success as use.
    fn report_op(
        &mut self,
        request: &Map<String, Value>,
        run: impl FnOnce(&Entry) -> std::result::Result<Value, Fail>,
    ) -> std::result::Result<Value, Fail> {
        let handle = text(request, "handle")?.to_owned();
        let entry = self
            .entries
            .get(&handle)
            .ok_or_else(|| missing(&self.evicted, &handle))?;
        if !entry.is_fresh() {
            return Err(fail(
                "stale",
                format!(
                    "handle {handle} is stale: {} changed or was removed; refresh it",
                    entry.path.display()
                ),
            ));
        }
        let report = run(entry)?;
        let revision = entry.revision;
        self.clock += 1;
        let stamp = self.clock;
        if let Some(entry) = self.entries.get_mut(&handle) {
            entry.last_used = stamp;
        }
        Ok(json!({"handle": handle, "revision": revision, "report": report}))
    }

    fn open(&mut self, request: &Map<String, Value>) -> std::result::Result<Value, Fail> {
        let path = PathBuf::from(text(request, "path")?);
        let loaded = load(&path)?;
        if self.entries.len() >= self.max_open {
            self.evict_least_recent();
        }
        self.clock += 1;
        let handle = format!("h{}", self.next_handle);
        self.next_handle += 1;
        let slides = loaded.slides;
        self.entries.insert(
            handle.clone(),
            Entry {
                path,
                meta: loaded.meta,
                package: loaded.package,
                session: loaded.session,
                revision: 1,
                last_used: self.clock,
            },
        );
        Ok(json!({"handle": handle, "revision": 1, "slides": slides}))
    }

    fn refresh(&mut self, request: &Map<String, Value>) -> std::result::Result<Value, Fail> {
        let handle = text(request, "handle")?.to_owned();
        let path = self
            .entries
            .get(&handle)
            .ok_or_else(|| missing(&self.evicted, &handle))?
            .path
            .clone();
        // A failed reload leaves the old snapshot and revision in place.
        let loaded = load(&path)?;
        self.clock += 1;
        let stamp = self.clock;
        let Some(entry) = self.entries.get_mut(&handle) else {
            return Err(missing(&self.evicted, &handle));
        };
        entry.meta = loaded.meta;
        entry.package = loaded.package;
        entry.session = loaded.session;
        entry.revision += 1;
        entry.last_used = stamp;
        Ok(json!({"handle": handle, "revision": entry.revision, "slides": loaded.slides}))
    }

    fn close(&mut self, request: &Map<String, Value>) -> std::result::Result<Value, Fail> {
        let handle = text(request, "handle")?;
        if self.entries.remove(handle).is_none() {
            return Err(missing(&self.evicted, handle));
        }
        Ok(json!({"handle": handle, "closed": true}))
    }

    fn evict_least_recent(&mut self) {
        // Stamps are unique, so the minimum is the single least recently used entry.
        let oldest = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(handle, _)| handle.clone());
        if let Some(handle) = oldest {
            self.entries.remove(&handle);
            self.evicted.insert(handle);
        }
    }
}
