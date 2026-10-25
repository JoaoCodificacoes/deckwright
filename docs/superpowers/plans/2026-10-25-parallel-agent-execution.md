# Deckwright parallel-agent execution plan

Date: 2026-10-25
Status: proposed dispatch plan. No agents started and no GitHub issue dependencies changed by this document.

**Goal:** Complete issues #1–#9 using one coordinator and up to three workers, while preserving the reference deck and the original correctness/performance gates.

**Architecture:** Keep the existing library, CLI and example architecture. Parallelize independently owned deliverables after their actual prerequisites are merged. Treat issue completion as an acceptance gate, rather than requiring every predecessor issue to close before any downstream preparation can begin.

**Tech stack:** Existing Rust/CLI plan, isolated Git worktrees, PowerShell PowerPoint oracle, generated test fixtures and the existing benchmark approach. No new orchestration service.

Governing plan: [thesis reconstruction and performance](2026-10-25-thesis-reconstruction-performance.md).
Repository: https://github.com/JoaoCodificacoes/deckwright

## 1. Capacity and responsibilities

Four total active slots: coordinator + three workers. Worker names below describe roles that can change between waves, not additional persistent agents.

Coordinator owns dispatch, interface decisions, shared files, integration, review, resource scheduling and final issue acceptance. Workers own narrowly scoped implementation, tests and evidence in isolated worktrees. Workers report to the coordinator and do not independently spawn agents or merge.

Use fresh worker context for each bounded assignment: the governing plan, relevant issue section, exact base commit, ownership paths, input contracts, acceptance cases and asset locations. Do not send the entire chat history. Keep the same model unless the user requests a change.

Prefer two useful workers to three workers blocked on the same dependency. Free a worker slot when an assignment finishes; use that slot for review if integration is the bottleneck.

## 2. Dependency corrections

GitHub currently says each issue depends on its immediate predecessor. Before execution, replace those dependency paragraphs with this distinction between earliest start and final acceptance. Retain all original acceptance criteria and milestone membership. This is a proposed scheduling change, not permission to skip validation.

| Issue | Earliest useful start | Requirements to close |
|---|---|---|
| #1 Reference/oracle | Immediately | Frozen source, coverage, oracle evidence and host definition |
| #2 Package/CLI | After initial workspace and inventory contract; synthetic fixtures can precede oracle results | #1 reference/host/evidence available, package preservation and performance gate pass |
| #3 Builder/first slides | After package editing interface, units and error contract are merged | #1/#2 gates and static slides 1–3 verified |
| #4 Timing/query | Oracle research during #1; compiler/evaluator implementation after snapshot, identity and timing contracts are merged | #3 plus animated demonstrator and query gate |
| #5 Complete static deck | After #3 stable builder and source-ID manifest; need not wait for all of #4 | All 25 static slides verified; shared source IDs remain compatible with #4 |
| #6 Complete animation/media | Media/emphasis engine work after #4; per-slide animation only after that slide's static source is merged | #4/#5 complete, every reference animation and media gate passes |
| #7 Checks/performance | Harness during #1; structural checks after snapshot contract; timing checks after #4 | #5/#6 complete and integrated correctness/performance matrix passes |
| #8 Hand-edit feedback | Identity survival research during #1; merge logic after stable identities, provenance and property access | #7 baseline plus conflict and rebuild tests on the complete deck |
| #9 Agent interface/package | Open/query session after #4 contract; check/diff adapters after their respective implementations | #7/#8 plus complete interface and fresh-install gate |

Partial starts never justify closing a parent issue. Issue numbers remain the tracking units; multiple bounded PRs may reference the same issue. Use “Refs #N” until the final acceptance PR can use “Closes #N”. Split out child issues only when an assignment genuinely needs independent tracking.

## 3. Work waves

### Wave 0 — Coordinator bootstrap

- [ ] Update the scheduling paragraphs in the governing plan and GitHub issues to reference this plan when execution begins.
- [ ] Add the minimum buildable workspace, module registration and test commands needed for the first assignments. No feature stubs that pretend to pass validation.
- [ ] Freeze the inventory JSON envelope, package part naming rules and typed error convention in a small reviewed interface change.
- [ ] Create a task ownership ledger containing worker, issue slice, base SHA, branch/worktree, files, required inputs and gate.
- [ ] Assign private input paths explicitly; ignored reference files do not automatically appear in a new worktree.

Gate: all workers can build the same base and know their owned paths and input locations.

### Wave 1 — Evidence, package, measurement

| Slot | Assignment | Owned paths / result |
|---|---|---|
| A | #1 reference inventory and PowerPoint oracle, including #8 identity-survival experiments | `oracle/`, reference coverage document and private evidence; executable capture protocol and identity findings |
| B | #2 package preservation, deterministic ZIP/XML operations and inspect/round-trip implementation | `src/package.rs`, `src/xml.rs`, package tests, inventory schema; working package APIs and command handlers |
| C | #7 benchmark harness preparation plus #1 benchmark-host record | `bench/`, benchmark documentation, generated stress-input generator; runnable measurement protocol using available binaries |

Coordinator integrates CLI wiring and manifests. A owns PowerPoint exclusively. C may prepare measurement tools while B builds; full-package metrics are pending until B supplies a release binary. Final performance measurements require an idle host. B can implement against neutral fixtures without waiting for A's video exports; #2 cannot close without the reference oracle check.

Exit: package contract merged, immutable reference available, host/environment documented, package gate passed.

### Wave 2 — Builder with research and independent validation

| Slot | Assignment | Owned paths / result |
|---|---|---|
| A | #3 units, resolved model, minimal builder and static slides 1–3 | Model/builder/units modules, authoring tests, slide sources 1–3 |
| B | #4 timing research and neutral oracle cases | Timing design document and generated timing-case inputs; submits PowerPoint jobs to oracle owner |
| C | #7 package/relationship checks and adversarial input tests | Separate check module and check tests; uses merged package APIs |

Coordinator folds identity research into stable authored-ID and source-provenance contracts before A stamps the initial slides. If no carrier passes, record a decision and keep dependent sync work blocked; do not manufacture reliability.

B can specify event-time semantics and expected traces while A works, but must not independently invent a competing resolved model. Compiler/evaluator implementation waits for the snapshot contract below. Existing oracle owner executes queued jobs when available; ownership transfers explicitly between waves.

Exit: static slides 1–3 verified; units, source IDs, property access and snapshot contract merged.

### Wave 3 — Animation core and first parallel slide batches

| Slot | Assignment | Owned paths / result |
|---|---|---|
| A | #4 timing compiler, state/Morph queries and slide 10 demonstrator | Timing/query modules and tests, state/Morph schemas, `slides/s10.rs` |
| B | #5 static slides 4–9 | Only `slides/s04.rs`–`s09.rs` and uniquely named batch tests |
| C | #5 static slides 11–14 and 19–25 | Only those slide source files and uniquely named batch tests |

Slide 10 belongs exclusively to A through acceptance of the demonstrator; #5 must reuse its merged source rather than author it again. Coordinator owns the asset manifest, example entrypoint, slide registry and shared builder feature requests. Missing primitives are implemented once by their owner, reviewed and merged before a slide batch uses them.

On completion, the first free slide worker takes static slides 15–18. It owns the video poster composition and media placement keys; animation/media engine integration remains a later handoff. Review and merge small batches so downstream work can start early.

Exit: #4 demonstrator verified. #5 batches may still be in progress. Never edit a slide's static and animation source concurrently.

### Wave 4 — Complete behavior and agent checks

| Slot | Assignment | Owned paths / result |
|---|---|---|
| A | #6 remaining engine capabilities: media call, emphasis/repeats and unsupported-state handling | Timing/query core and focused engine tests; no slide files currently leased to B |
| B | Finish #5 static batches, then port #6 timing in accepted slide sources | Exclusively assigned slide files; reference-to-generated effect mapping |
| C | #7 structural/timing checks and #9 basic open/query session support | Check module, check schema and a dedicated session module; no CLI entrypoint edits |

Coordinator implements or assigns shared wiring in short integration changes. A provides stable effect APIs before B consumes them. Completed static batches can be handed to B for animation without waiting for unrelated batches. Morph validation covers adjacent slide pairs, so transitions spanning batches are verified only after both sides are integrated.

Exit: all 25 static slides and all reference behaviors integrated; checks and query session ready for final qualification.

### Wave 5 — Qualification, bounded sync and interface completion

| Slot | Assignment | Owned paths / result |
|---|---|---|
| A | #8 three-way diff/overrides | Sync module, sync tests and diff schema; uses frozen identity and property contracts |
| B | #9 session transport/MCP and packaging | Interface adapter files, packaging docs and neutral example; diff adapter waits for A's merged API |
| C | #7 independent fidelity and performance qualification | Evidence and reproducible findings against a pinned integration SHA |

Run C's final measurements in a quiet reservation: pause builds, tests, PowerPoint exports and other heavy workers on the same machine. Workers can prepare code outside that window. Qualification is tied to a SHA and environment; after A/B integrate, recheck affected paths and run the final integrated acceptance suite before release. A result from an earlier SHA is not release evidence for later code.

Exit: #7, #8 and #9 close only after their full acceptance gates, including new source vs hand-edit conflicts and fresh installation, pass on the final integration revision.

## 4. Contracts required before dependent dispatch

Keep contracts in the actual public Rust types plus short documentation. Do not add a parallel schema/type framework just to coordinate agents.

| Contract | Freeze before | Required decisions |
|---|---|---|
| Package | #3 implementation | Part path normalization, relationship addressing, borrowed/owned bytes, preservation boundary, deterministic writer and typed errors |
| Builder/identity | Parallel slide authoring | Explicit source keys, separate Morph names, asset keys, slide index/order, group transforms and extension request process |
| Snapshot | #4 evaluator and resolved checks | Shape identity/index lookup, resolved geometry/style, immutable ownership, revision and unknown property representation |
| Timing | Compiler/evaluator split or downstream #6 | Event history, absolute sample time, trigger ordering, overlap, repeat boundaries and unsupported constructs |
| Reports/session | #9 transport | Versioned JSON envelopes, revision on results, invalid/stale handle errors and owned cache lifetime |
| Sync | #8 implementation | Baseline/current/edited roles, per-property provenance, equal-edit behavior, duplicate IDs, ambiguous/deleted/added-object reporting |

Any contract change is proposed to the coordinator first. The coordinator identifies consumers, lands the contract change with tests, and dispatches consumer updates. Workers do not patch neighboring modules opportunistically.

## 5. Ownership and isolation

- One worktree and branch per active assignment, based on an exact reviewed commit. Check existing managed worktrees before creating more.
- Worktrees isolate source edits, not the machine: they still share RAM/CPU, PowerPoint and possibly build caches.
- Coordinator exclusively owns workspace manifests, lockfile, library module registration, CLI entrypoint, example registry and shared asset manifest. Workers request small wiring changes with exact declarations.
- Assign test files and fixtures uniquely per worker; do not have all agents append to one integration-test file.
- Oracle output uses per-run directories keyed by input hash and revision. Only one PowerPoint automation owner at a time; never kill the user's unrelated PowerPoint processes.
- Give each worktree its own build output directory to avoid build-lock contention. Limit simultaneous heavy builds based on host capacity.
- Keep source media read-only and refer to the agreed local path/hash. Do not duplicate private assets into tracked worktree directories.
- Worker handoff includes commit SHA, files changed, test commands/results, benchmark context, uncertainties and required downstream changes.
- Coordinator reviews and integrates one completed change at a time, then updates the base SHA for the next dependent assignment. No force-pushing shared branches.
- Failed gates return to the owning worker with a bounded fix. Do not weaken a test or acceptance criterion to keep the wave moving.

## 6. Dispatch template

Use the following fields for each actual assignment; the coordinator supplies concrete values before dispatch:

1. Issue and bounded deliverable, such as “#5 static slides 4–9”.
2. Exact base SHA, absolute worktree directory and branch.
3. Allowed source/test paths and coordinator-owned files that cannot be edited.
4. Merged input contracts, relevant documentation and private asset manifest path/hash.
5. Expected output and explicit passing/failing cases from the governing issue.
6. Verification commands valid at that base revision, with expected outcomes.
7. Resource restrictions: PowerPoint queue, build concurrency and benchmark reservations.
8. Handoff requirements: commit, evidence and any blockers. No independent merges, issue closure, nested delegation or unrelated changes.

A worker blocked on an API reports the missing contract and stops dependent edits. The coordinator can reassign it to a ready independent task; guessing interfaces is not progress.

## 7. Integration and issue completion checklist

- [ ] Review ownership violations and source diff.
- [ ] Confirm tests exercise behavior and preservation, not only snapshots of implementation.
- [ ] Integrate on the declared base and run the relevant combined tests.
- [ ] Verify affected slide batches and incoming/outgoing Morph boundaries.
- [ ] Run required oracle jobs with fixed click schedules and recorded environment.
- [ ] Run final performance checks without competing workloads.
- [ ] Attach evidence to the parent issue; keep partial acceptance items open.
- [ ] Close only issues whose complete gate passes; retain unresolved fidelity/identity risks explicitly.

## 8. Critical path and first dispatch

The principal dependency path is package interface → minimal builder/identity → timing/query → complete slide behavior → integrated oracle/performance gate. Sync and interface work can overlap the later stages once their contracts stabilize.

Most useful parallelism comes from separating oracle work from package implementation, preparing measurement/check tooling early, and splitting slide reconstruction by owned files. Shared core API changes and same-machine validation remain serial. Do not promise a fixed speedup before measuring work duration and coordination overhead.

First dispatch after bootstrap:

- Worker A: #1 oracle/reference coverage and identity experiments.
- Worker B: #2 package preservation and inspect/round-trip core.
- Worker C: #7 benchmark tooling and #1 host baseline.
- Coordinator: contracts, shared wiring, review and resource queue.

No implementation workers are launched as part of writing this plan.
