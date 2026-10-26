use deckwright::{Result, model::Slide};
use std::path::Path;
/// Institution overview, retaining native text, cards and original pictograms.
pub fn build(source: &Path) -> Result<Slide> {
    crate::read_slide(source, "s02.json")
}
