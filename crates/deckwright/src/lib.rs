//! Deterministic no-op OPC package import/export and structural inventory.
//!
//! `Package` owns uncompressed part bytes; `part` borrows them unchanged. It does
//! not evaluate animations or rewrite XML. External links are classified only.
//! Canonical part names are relative slash-separated paths; traversal, URI query,
//! percent-encoded ZIP names, ZIP64 and multi-disk archives are rejected explicitly.
//! Relationship targets support UTF-8 and percent-encoded UTF-8, relative to the
//! source part. `write` creates a new file and refuses existing output paths.
//! Default limits are 128 MiB input/part, 512 MiB total expansion, 10,000 parts,
//! and 256 XML levels. Callers may supply smaller limits at the trust boundary.
//! Inventory timing trees describe encoded OOXML, not validated playback.

pub mod builder;
pub mod deck;
pub mod error;
pub mod model;
pub mod package;
pub mod units;
pub mod xml;
pub use error::{Error, Result};
pub use package::{Limits, Package};
pub mod check;
pub mod query;
pub mod session;
pub mod table;
pub mod template;
pub mod timing;
