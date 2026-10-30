# Morph demo

A three-slide, 16:9 deck that shows Morph matching by explicit `!!` name,
built from typed Rust data with no external input.

- **Slide 1:** a title bar and a rounded box named `!!box`.
- **Slide 2:** `!!box` moves right and grows (Morph transition, 2000 ms). A
  rounded `dot` fades in on click over 800 ms.
- **Slide 3:** `!!box` moves and shrinks (Morph transition, 2000 ms). A
  rectangle `arrow` slides in from the left on click over 1500 ms, then fades
  out 500 ms after it finishes, over 700 ms.

The deck starts from `deckwright::template::blank`, a generated template with
fixed neutral metadata and no private assets, themes, or media. The slides are
compiled with `compile_animated_slide` and swapped in with `Package::edited`.

## Run

```sh
cargo run -p deckwright --example morph_demo -- demo.pptx
```

The output path must not exist. The command prints a JSON summary with the
slide count, the `!!box` Morph delta from slide 1 to slide 2, and the number
of `dw check` errors.

## Inspect

```sh
dw query morph demo.pptx --from 1 --to 2 --json
dw query state demo.pptx --slide 2 --at-ms 0 --clicks-ms 0 --json
dw check demo.pptx --json
```

`dw query morph` should report the `!!box` match. `dw query state` at a click
time shows the `dot` becoming visible. `dw check` should report no errors.
PowerPoint playback is not verified by these tools.

The example is neutral and contains no private assets, so it is safe to publish.
