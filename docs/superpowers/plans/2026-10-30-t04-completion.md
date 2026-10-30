# T04 completion and fast-worker parallelization plan

Date: 2026-10-30
Status: proposed. Refs #4. Base: `main` 3404107; open draft PR #17 (`agent/issue4-query`, head 732cc6d).

Governing plans: [thesis reconstruction](2026-10-25-thesis-reconstruction-performance.md), [parallel-agent execution](2026-10-25-parallel-agent-execution.md). All T04 acceptance criteria and gates remain unchanged.

## 1. Where T04 stands

Done on PR #17 (not merged):

- Shared typed timing contract, `validate_timeline`, `parse_slide_timing`, `compile_animated_slide` (`timing.rs`).
- `QuerySession` with `state`, `timeline` and `morph`, explicit `!!` Morph links, opaque-timing propagation, group geometry (`query.rs`).
- Versioned `state-v1`, `timeline-v1` and `morph-v1` schemas, `docs/decisions/{timing,query}.md`.
- 35 library tests pass locally and on CI.

The PR's CI is red for one reason only: `clippy::collapsible_if` at `crates/deckwright/src/timing.rs:117` (an `if let` with a nested `if`; current stable clippy requires a let-chain). fmt and all tests pass.

Missing against the T04 checklist:

| Checklist item | Gap |
|---|---|
| `dw query state/timeline/morph` | No CLI subcommands; `dw-cli` only has inspect/roundtrip/check. |
| Rebuild slides 1–3 with transitions/builds | Example compiles static slides only; no typed timelines in `s01.rs`–`s03.rs`. |
| Slide 10 motion/exit demonstrator | `slides/s10.rs` does not exist. |
| Oracle cases (presets 10/22, exit 10, motion, with/after/click, Morph links) | COM engine traces exist; rendered click recordings are explicitly pending in `oracle/README.md`. |
| Benchmark query evaluation + serialization | No `benches/`, no query workloads in `bench/run.py`. |
| Docs | `timing.md` still says the parser/compiler are "planned"; README says queries are a later milestone. |

## 2. T04 completion plan

Work items, in dependency order. Each ends with its own commit and verification.

### Step 0 — Unblock PR #17 (coordinator)

- Collapse the `if let … { if … }` at `timing.rs:117` into a let-chain. No behavior change.
- Merge `main` into the PR branch if it has moved (merge commit, no rebase).
- Verify: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --all-targets --locked`.
- Freeze two contracts in `docs/decisions/query.md` before dispatching consumers:
  - **CLI contract.** `dw query state <pptx> --slide N --clicks-ms a,b,… --at-ms T --json`, `dw query timeline <pptx> --slide N --json`, `dw query morph <pptx> --from N --to M --json`. Exit 0 on a report, 1 on error, with a usage error for malformed arguments. stdout carries exactly one JSON report followed by a newline.
  - **Canonical timeline form.** The signature of a `timing::canonicalize(&Timeline) -> Timeline` normalizer: sorted stages and effects, resolved implicit defaults, merged equivalent delays. Equality of canonical forms is the timing-equivalence check for rebuilt vs reference slides.

### Step 1 — Query CLI (parallel)

- Wire the three subcommands through `QuerySession::open` and the existing report types. No new evaluation logic in the CLI.
- Tests in a new `crates/dw-cli/tests/query_cli.rs`, using a generated neutral fixture: valid reports, schema validation, future-click error, out-of-range slide, missing flags, nondecreasing-clicks violation.

### Step 2 — Differential evaluator test (parallel)

- New `crates/deckwright/tests/query_differential.rs`: a deliberately naive reference evaluator that walks stages and effects one event at a time.
- Generate random valid timelines and click histories (`proptest`, dev-dependency added by the coordinator). Assert visibility, opacity, wipe and motion offset agree with `QuerySession::state` within 1e-9.
- Any disagreement is a bug report to the timing owner, never a loosened assertion.

### Step 3 — Query benchmarks (parallel)

- In-process: `crates/deckwright/benches/query.rs` (`harness = false`, std `Instant`, 5 warmups and 50 samples). It covers state evaluation + JSON serialization and Morph diff + serialization against the 5 ms budgets, emitting JSON with raw samples.
- Subprocess: add `query state` workloads to `bench/run.py` for the 300 ms CLI budget, with tests in `bench/test_run.py`.
- Record bootstrap confidence intervals next to p95 (see the comment on #7). The gate rule (>10% and >2 ms vs frozen baseline) is unchanged.

### Step 4 — Canonical form and timing equivalence (after Step 0 freezes the signature)

- Implement `timing::canonicalize` with unit tests: idempotence, order independence of with-previous siblings, and default resolution.
- Add an equivalence helper used by the example: parse reference slide timing and rebuilt slide timing, canonicalize both, compare. Neutral fixtures run in public CI; the thesis comparison runs locally.

### Step 5 — Animated slides 1–3 and slide 10 (local host with private assets)

- Author typed `Timeline`s in `s01.rs`–`s03.rs` from the inventory and engine traces. Compile with `compile_animated_slide`. Never copy timing XML.
- Extract and author static `s10.rs`, then its motion/exit timeline. Slide 10 stays owned by T04 until the demonstrator is accepted (parallel plan, wave 3).
- Verify static PNG equality is unchanged and canonical timing equals the reference for slides 1–3 and 10. Mutation test: changing one delay in source changes exactly that stage.

### Step 6 — Oracle verification (Windows host with PowerPoint)

- Run neutral calibration cases for entrance presets 10/22, exit 10, relative motion, with/after/click, and explicit Morph links.
- Then run rendered click-scheduled recordings for slides 2–3 and 10 per `oracle/README.md`.
- Compare sampled query states against the recorded event schedule with declared tolerances.
- If rendered recording is still unavailable, the T04 gate stays open as a pending behavior-validation gate. Engine traces are not a substitute.

### Step 7 — Docs and close-out (coordinator)

- Update `timing.md` (remove "planned"), `query.md` (CLI contract), README status and try-it commands.
- Attach evidence to #4: test output, benchmark JSON with host provenance, oracle run IDs.
- Close #4 only when every checklist item and the gate pass. Otherwise move PR #17 out of draft with "Refs #4" and keep the issue open.

## 3. Parallelization with fast worker agents

### Ground rules

- **Capacity.** One coordinator plus at most 3 concurrent fast workers. Prefer 2 unblocked workers over 3 blocked ones.
- **Isolation.** One git worktree and branch per assignment, branched from the exact integration SHA. Each worktree gets its own `CARGO_TARGET_DIR` to avoid build-lock contention.
- **Coordinator-owned files.** `Cargo.toml`, `Cargo.lock`, `crates/deckwright/src/lib.rs`, `examples/thesis/main.rs`, schemas and `docs/decisions/*`. A worker that needs a change there reports the exact declaration and stops.
- **Leases.** A file outside that list belongs to exactly one worker per wave, named in its assignment.
- **Fast workers suit** well-specified, test-first, mechanical tasks with an exact file list and exact verification commands. They do not design contracts, resolve semantic ambiguity, or touch PowerPoint or private assets.
- **Diff size.** Each assignment is at most ~300 changed lines. Larger work is split before dispatch.
- **Handoff.** Every worker returns its branch, commit SHA, files changed, the verbatim output of each verification command, and open questions. No merges, PRs, issue edits or nested agents.

### Waves

| Wave | Slot | Assignment | Owned paths | Depends on |
|---|---|---|---|---|
| 0 | Coordinator | Step 0: clippy fix, merge main, freeze CLI + canonical contracts, add `proptest` dev-dep and the bench target | coordinator files, `timing.rs:117` | — |
| 1 | W1 | Step 1: query CLI + tests | `crates/dw-cli/src/main.rs` (leased), `crates/dw-cli/tests/query_cli.rs` | Wave 0 |
| 1 | W2 | Step 2: differential evaluator test | `crates/deckwright/tests/query_differential.rs` | Wave 0 |
| 1 | W3 | Step 3: benchmarks | `crates/deckwright/benches/query.rs`, `bench/run.py`, `bench/test_run.py` | Wave 0 |
| 2 | W1 | Step 4: `timing::canonicalize` + tests | `crates/deckwright/src/timing.rs` (leased), `crates/deckwright/tests/timing_canonical.rs` | Wave 1 merged |
| 2 | W2 | Fix any W2 differential findings in `query.rs`, with a regression test per finding | `crates/deckwright/src/query.rs` (leased) | W2 findings |
| 2 | W3 | Schema conformance test for CLI output, docs draft for Step 7 | `crates/dw-cli/tests/query_schema.rs`, README draft returned as a patch | Wave 1 merged |
| 3 | Local | Steps 5–6 on the host with private assets and PowerPoint | `examples/thesis/slides/s01–s03.rs`, `s10.rs`, `oracle/` runs | Wave 2 merged |

Wave 3 cannot run in a cloud container: the private thesis assets and PowerPoint exist only on the Windows host. Run it from a local session (or a cloud session attached to that machine), one PowerPoint owner at a time.

### Integration loop (coordinator)

1. Review the worker diff against its lease. Reject edits outside owned paths and any weakened assertion.
2. Merge it into an integration branch off PR #17's head, one worker at a time.
3. Run fmt, clippy `-D warnings`, the full workspace tests, and the Python unit tests for `bench/` when touched.
4. Push to PR #17's branch only after it passes clean. Update the base SHA for the next wave.
5. A failure goes back to the owning worker with the exact output, as a bounded fix.

### Dispatch prompt template

```
Task: <issue + step>, e.g. "#4 Step 2: differential evaluator test".
Worktree: <path>  Branch: <branch>  Base SHA: <sha>  CARGO_TARGET_DIR=<dir>
You may edit only: <files>. Do not edit: Cargo.toml, Cargo.lock, lib.rs, schemas, docs/decisions.
Read first: docs/decisions/timing.md, docs/decisions/query.md, <relevant src files>.
Contract (frozen, do not change): <signatures / CLI grammar>.
Required cases: <explicit passing and failing cases>.
Write the tests first and show them failing, then implement.
Verify with exactly: cargo fmt --all --check; cargo clippy --workspace --all-targets --locked -- -D warnings; cargo test --workspace --all-targets --locked
If a contract is missing or ambiguous, stop and report it. Do not guess an interface.
Commit on your branch. Return: SHA, files changed, verbatim command output, open questions.
```

### Expected critical path

Wave 0 → Wave 1 (three workers in parallel) → Wave 2 → local Steps 5–6 → Step 7.
Waves 0–2 are cloud-parallel. Steps 5–6 are serial on the PowerPoint host and are T04's true bottleneck. Start the neutral oracle calibration (Step 6, first part) on that host as soon as Wave 0 lands, in parallel with Waves 1–2.
