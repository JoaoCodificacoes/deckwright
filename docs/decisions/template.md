# Neutral blank template, version 1

`deckwright::template::blank(slides: usize, width: Emu, height: Emu) ->
Result<Package>` builds a minimal, complete PresentationML package in memory,
with no external input. It exists for public examples and CI fixtures that
must never include private assets.

**Parts.**
- `[Content_Types].xml` and `_rels/.rels`;
- `docProps/core.xml` and `docProps/app.xml`, with fixed neutral metadata and
  no dates;
- `ppt/presentation.xml` with `sldMasterIdLst`, `sldIdLst` (IDs from 256),
  `sldSz` and `notesSz`, plus its rels;
- `ppt/presProps.xml`, `ppt/viewProps.xml` and `ppt/tableStyles.xml`;
- `ppt/theme/theme1.xml`: a complete theme with a color scheme, a font scheme
  using `Calibri` for both major and minor fonts, and a format scheme with
  three fill, line and effect styles plus background fills;
- `ppt/slideMasters/slideMaster1.xml`, with a title, body and text styles,
  related to the theme and the layout;
- `ppt/slideLayouts/slideLayout1.xml` of type `blank`, related to the master;
- `ppt/slides/slideN.xml` for N in 1..=slides, each with an empty `spTree`
  and related to the layout.

**Contract.**
- The output is deterministic: identical inputs give identical bytes.
- `slides` is 1..=1000, and `width`/`height` follow the PowerPoint `sldSz`
  bounds (914400..=51206400). Anything else is an `InvalidPart` error.
- The result passes `Package::from_bytes` and `check::check_package` with no
  error findings. `QuerySession::from_package` opens it with the requested
  slide count and size.
- Slides are replaced with `Package::edited` using
  `timing::compile_animated_slide` output. The slide rels already point at the
  layout, so authored shapes need no further relationship edits.

PowerPoint opening and repair-free saving are verified by the oracle on the
Windows host. LibreOffice rendering is the cloud-side proxy.
