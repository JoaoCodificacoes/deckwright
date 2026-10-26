use deckwright::{Result, model::Slide};
use std::path::Path;
/// Workflow and editable form groups with their original coordinate systems.
pub fn build(source: &Path) -> Result<Slide> {
    crate::read_slide(source, "s03.json")
}
