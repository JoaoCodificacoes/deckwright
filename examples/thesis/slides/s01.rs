use deckwright::{Result, model::Slide};
use std::path::Path;
/// Cover content, as editable typed shapes, connector and original image.
pub fn build(source: &Path) -> Result<Slide> {
    crate::read_slide(source, "s01.json")
}
