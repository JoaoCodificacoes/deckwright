use deckwright::{Limits, Package, query::QuerySession, session::Server};
use std::io::{BufRead, Write};
use std::{collections::HashMap, str::FromStr};
const USAGE: &str = "usage: dw inspect <pptx> --json | dw roundtrip <input> --output <new-output> | dw check <pptx> --json | dw query state <pptx> --slide <N> --at-ms <T> [--clicks-ms <t1,t2,...>] --json | dw query timeline <pptx> --slide <N> --json | dw query morph <pptx> --from <N> --to <M> --json | dw serve [--max-open <N>]";
fn uint<T: FromStr>(name: &str, s: &str) -> Result<T, String> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{name} expects a base-10 integer, got {s:?}"));
    }
    s.parse()
        .map_err(|_| format!("{name} is out of range: {s}"))
}
fn required<'a>(values: &HashMap<&str, &'a str>, name: &str) -> Result<&'a str, String> {
    values
        .get(name)
        .copied()
        .ok_or_else(|| format!("missing required {name}"))
}
// Takes a fully serialized report so a failure never leaves partial stdout.
fn emit(text: serde_json::Result<String>) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", text?);
    Ok(())
}
fn query(sub: &str, input: &str, rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let valued: &[&str] = match sub {
        "state" => &["--slide", "--at-ms", "--clicks-ms"],
        "timeline" => &["--slide"],
        "morph" => &["--from", "--to"],
        _ => return Err(USAGE.into()),
    };
    let mut values = HashMap::new();
    let mut json = false;
    let mut i = 0;
    while i < rest.len() {
        let flag = rest[i].as_str();
        if flag == "--json" {
            if json {
                return Err("duplicate flag --json".into());
            }
            json = true;
            i += 1;
        } else if valued.contains(&flag) {
            let value = rest
                .get(i + 1)
                .ok_or_else(|| format!("missing value for {flag}"))?;
            if values.insert(flag, value.as_str()).is_some() {
                return Err(format!("duplicate flag {flag}").into());
            }
            i += 2;
        } else {
            return Err(format!("unknown argument {flag}").into());
        }
    }
    if !json {
        return Err("--json is required".into());
    }
    let session = QuerySession::open(input, Limits::default())?;
    match sub {
        "state" => {
            let slide = uint("--slide", required(&values, "--slide")?)?;
            let at = uint("--at-ms", required(&values, "--at-ms")?)?;
            let clicks = match values.get("--clicks-ms") {
                None => vec![],
                Some(list) => list
                    .split(',')
                    .map(|t| uint::<u64>("--clicks-ms", t))
                    .collect::<Result<Vec<_>, _>>()?,
            };
            emit(serde_json::to_string(&session.state(slide, &clicks, at)?))
        }
        "timeline" => {
            let slide = uint("--slide", required(&values, "--slide")?)?;
            emit(serde_json::to_string(&session.timeline(slide)?))
        }
        _ => {
            let from = uint("--from", required(&values, "--from")?)?;
            let to = uint("--to", required(&values, "--to")?)?;
            emit(serde_json::to_string(&session.morph(from, to)?))
        }
    }
}
// One response line per request line; each line is flushed before the next is read.
fn serve(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let max_open = match rest {
        [] => 8,
        [flag, value] if flag == "--max-open" => {
            let n: usize = uint("--max-open", value)?;
            if n == 0 {
                return Err("--max-open must be at least 1".into());
            }
            n
        }
        _ => return Err(USAGE.into()),
    };
    let mut server = Server::new(max_open);
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        writeln!(out, "{}", server.handle_line(&line?))?;
        out.flush()?;
    }
    Ok(())
}
fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    match a.as_slice() {
        [cmd, input, json] if cmd == "inspect" && json == "--json" => {
            let p = Package::open(input, Limits::default())?;
            serde_json::to_writer(std::io::stdout().lock(), &p.inventory())?;
            println!();
        }
        [cmd, input, flag, output] if cmd == "roundtrip" && flag == "--output" => {
            if std::path::Path::new(input).canonicalize().ok()
                == std::path::Path::new(output).canonicalize().ok()
                && std::path::Path::new(output).exists()
            {
                return Err("input and output must differ".into());
            }
            Package::open(input, Limits::default())?.write(output)?;
        }
        [cmd, input, json] if cmd == "check" && json == "--json" => {
            let report = deckwright::check::check_path(input, Limits::default());
            serde_json::to_writer(std::io::stdout().lock(), &report)?;
            println!();
            return Ok(i32::from(report.has_errors()));
        }
        [cmd, sub, input, rest @ ..] if cmd == "query" => query(sub, input, rest)?,
        [cmd, rest @ ..] if cmd == "serve" => serve(rest)?,
        _ => return Err(USAGE.into()),
    }
    Ok(0)
}
fn main() {
    match run() {
        Ok(0) => {}
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("dw: {e}");
            std::process::exit(1);
        }
    }
}
