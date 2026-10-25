# Stable authored identity: provisional decision

Status: provisional; PowerPoint survival matrix is an open Task 8 gate.

Keep source keys in an external authored manifest. Numeric cNvPr IDs are slide-local package references, not durable source keys. Preserve existing cNvPr names used for explicit Morph matching separately; never overwrite names such as !!title with authored identity.

Candidate in-package carriers are shape tags and a dedicated extension payload; neither is accepted until tested. A manifest-only geometric/text remapper cannot resolve duplicate or removed objects reliably. Ambiguity must be reported instead of silently reassigned.

Required neutral-fixture matrix for each carrier: save/reopen, move/resize, copy/paste within/across slides, duplicate slide, group/ungroup. Compare source key, cNvPr ID, Morph name and carrier value after each action. Distinguish survival from uniqueness: duplicated objects may retain a carrier and require duplicate-identity errors. Record Office build, fixture/script hashes and before/after package evidence.

Current environment: COM activation succeeded (PowerPoint 16.0 build 20430). No survival claims have been established. The original thesis is never an identity experiment fixture. Identity carrier finalization and hand-edit synchronization remain blocked on the neutral-fixture matrix, even if static export succeeds.

Observed neutral COM experiment (PowerPoint 16.0 build 20430): shape tag `deckwright.source=neutral.shape.one` and name `!!neutral-morph` survived save/reopen and position change. Shape duplication retained both tag and Morph name but changed numeric ID 2 to 3. Slide duplication retained tag/name and slide-local numeric ID 2. Therefore tag survival does not establish uniqueness; duplicate detection is mandatory. Copy/paste failed with an empty/unsupported clipboard in headless COM. Group/ungroup lookup failed with 0x800A01A8; neither has a survival result. Private evidence: `fixtures/private/identity-run-4/identity.json`. Reproduce the neutral save/move/duplicate subset with `oracle/identity.ps1` and a fresh output directory. Extension carriers were not tested. The tag carrier remains provisional until the missing operations are verified.
