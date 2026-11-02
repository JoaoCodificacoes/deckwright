# Deckwright and python-pptx

Both projects work with editable `.pptx` files, but they offer different entry points. Choose based on the task and the APIs you need, then test the resulting deck in PowerPoint.

| Task | Deckwright today | python-pptx |
| --- | --- | --- |
| Script common slide content | Typed Rust model for slides, shapes, text, pictures, tables, and charts; see the [Morph demo](../examples/morph_demo/main.rs) and [authoring model](decisions/authoring.md) | Python API for slides, placeholders, text, images, shapes, tables, and charts; see its [feature list](https://python-pptx.readthedocs.io/en/latest/) |
| Preserve an existing presentation | Bounded OPC package import/export; `Package::edited` replaces selected parts and retains others | Opens and saves existing presentations, including parts it cannot directly manipulate; see [working with presentations](https://python-pptx.readthedocs.io/en/latest/user/presentations.html) |
| Work with animation data | Compiles a supported timing model; CLI and MCP can report state, timeline, and unique explicit `!!` Morph pairs | Its documented user guide focuses on slide content. Check its current API and your deck before choosing it for an animation workflow. |
| Run automated diagnostics | `dw check` returns supported package, timing, and bounds findings, with explicit limitations | Use the library's object API and your own validation for the checks your workflow needs. |
| Agent interface | `dw-mcp` exposes read and check tools over stdio | A Python agent can call the library directly. |

Deckwright's animation reports are **models of supported OOXML behavior**, not a PowerPoint renderer. Unknown timing remains unknown, and automatic Morph object pairing is outside the current query contract. Neither this comparison nor a successful static render establishes playback fidelity. See [query semantics](decisions/query.md) and [MCP integration](agent-integration.md) for the current boundary.

For a Python workflow centered on ordinary slide generation, start with the mature [python-pptx user guide](https://python-pptx.readthedocs.io/en/latest/user/quickstart.html). For Rust authoring plus inspectable timing and explicit Morph matching, try Deckwright's [neutral demo](../examples/morph_demo/README.md). If a presentation depends on transitions or font layout, verify it in PowerPoint before delivery.
