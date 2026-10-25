# deckwright

A performance-focused Rust framework for authoring editable PowerPoint decks and inspecting animation state as data.

The first release will faithfully reconstruct the 25-slide thesis reference, including its original assets, Morph transitions, click builds, motion paths, emphasis effects and embedded video.

## Plan

See [the implementation plan](docs/superpowers/plans/2026-10-25-thesis-reconstruction-performance.md) for scope, acceptance criteria and proposed performance budgets.

Initial targets on a fixed benchmark host: full build under 1 second, warm single-slide edit under 250 ms, warm state query under 5 ms, and core peak memory under 128 MiB. These are engineering targets, not measured results.

## Status

Planning and issue setup. The Rust framework has not been implemented yet.

## Reference assets

The thesis deck, extracted media, detailed slide inventory and private oracle evidence remain local and are excluded from Git. Implementation must preserve original wording and visuals and keep native slide objects editable. Use generated neutral fixtures in public CI.

## Development

Work through the numbered issues in dependency order. Each issue includes its scope and acceptance gate. Record correctness and performance evidence before marking work complete.
