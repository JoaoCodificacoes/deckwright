# PowerPoint reference oracle

Supervision uses `worker.ps1` for the export, identity and observation harnesses. It starts a hidden, owned PowerShell child with encoded JSON arguments and waits for a bounded interval. On timeout it terminates only that exact child process, waits at most two more seconds and writes `worker-process.json`; no process-tree kill, PowerPoint termination or Application.Quit occurs. An interrupted COM call can leave an owned presentation open and cleanup is explicitly unverified. A zero child exit also requires the caller's completion JSON and accepted status; missing or failed completion metadata is rejected. The former Start-Job/Stop-Job approach was observed hanging on blocked COM, so it is no longer used. `pwsh -NoProfile -ExecutionPolicy Bypass -File oracle/test_worker.ps1` verifies success/argument fidelity, zero-exit metadata guards and a sleeping neutral child timeout without invoking Office.

Neutral scheduled-dispatch calibration (Windows PowerShell 7):

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File oracle/observe.ps1 -OutputDirectory fixtures/private/neutral-observation
pwsh -NoProfile -ExecutionPolicy Bypass -File oracle/test_observation.ps1 -EvidencePath fixtures/private/neutral-observation/observation.json
pwsh -NoProfile -ExecutionPolicy Bypass -File oracle/fonts.ps1 -OutputPath fixtures/private/fonts.json
```

Observation uses numbered entrance effects and monotonic slide-entry/before/after Next timestamps with actual slideshow slide/click indices. It is bounded by a parent timeout and closes only its own show/presentation. No screenshots, rendered-frame capture or frame-rate claim is made. Engine progression does not establish visible animation timing; reference click schedules remain gated on independent rendered observations. The neutral deck is generated and hashed, and no private reference is opened. Font inventory records installed families/files; assignment and glyph substitution remain distinct evidence categories. Identity now also tests numeric range grouping/ungrouping and final save/reopen, under a timeout; clipboard and extension-carrier gates stay pending.

Optional read-only source traces use `-Source <frozen-copy.pptx> -TimeoutSeconds 180` and the default selected slides 2–3, 9–12, 15–18 and 21–22. The source hash is checked before copying; source and staged hashes are checked inside the worker before Office and after cleanup. Only the staged copy is opened. `-Slides` accepts a PowerShell integer array; `-MaximumClicksPerSlide` bounds dispatch (default 10). After GotoSlide, a fixed 2500 ms observation delay permits click-state initialization during Morph; it does not prove visual transition completion. Every COM observation field has its own error record: for example slide 15 can reject GetClickIndex while reporting zero clicks. A successful supervised run means the trace completed, not that every field or feature was observable. Media playback, actual visible effects and complete scheduling remain separate gates. Keep the neutral calibration evidence alongside source traces.

`python3 oracle/compare_static.py reference-office.json authored-office.json --output new-comparison.json` compares corresponding leading slides' native top-level COM objects and PNG hashes. It reports both total counts and unpaired slide indices, including when the authored deck is longer. It does not compare nested shape details, package assets or animation semantics; use independent package evidence for those.

Run from Windows PowerShell 7 with a fresh ignored output directory:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File oracle/run.ps1 -Source C:/Users/joaod/Downloads/Joao_Thesis_Presentation.pptx -OutputDirectory fixtures/private/oracle-run
```

The default expected hash freezes the plan input. A mismatch stops before copying or opening it. Never override the expected hash without recording a reference change decision. Outputs include a separate reference copy, static 1920 × 1080 PNGs, Office version/build, input/script hashes and export hashes. The source is opened only through the separate copy, read-only; no save is performed. Fresh output directories prevent accidental replacement of prior evidence.

The worker is bounded by a parent timeout. Cleanup closes only its presentation and releases COM objects. It never kills PowerPoint or calls Application.Quit because COM activation may share an existing user instance. A timeout may leave a read-only copied presentation open; inspect it manually. COM failure is an explicit pending gate. Fonts/substitutions remain unverified; static PNGs do not establish click playback, autoplay, repeat or Morph semantics.

Package-only inventory (Python 3 standard library, separate from core Rust runtime):

```sh
python3 oracle/inventory.py /path/reference.pptx --output fixtures/private/inventory.json
python3 -m unittest discover -s oracle -v
```

Inventory follows presentation relationships for order and retains raw slide, shape, timing, transition and notes XML. This preserves geometry, text run styles, crop, custom geometry, timing triggers/repeats/delays and asset relationships as inspectable evidence without interpreting their behavior. Namespace prefixes in serialized subtrees may differ; raw slide XML is retained separately. This trusted-local utility is not a hardened arbitrary-package parser or a runtime benchmark.

Click recording is deliberately pending. Before implementation, validate slide-entry and click-event dispatch against visible numbered neutral effects, record a monotonic event schedule, capture frame timestamps/rate and sample expectations, and verify media/repeat cases. PowerPoint CreateVideo alone does not prove click scheduling. Required first recordings: slides 2–3, 9–12, 15–18, 21–22; all remaining slides before their reconstruction gate. Render comparison also requires installed-font inventory and calibrated geometry/line-break checks.

Current frozen hash in run.ps1 reflects the user's explicitly authorized 2026-10-25 replacement; the old and new hashes are recorded in this README's history and the closing evidence on #1. Identity experiment is a generated neutral deck and excludes clipboard/group operations after those attempts failed in this environment. It tests tag retention and exposes duplicate-key behavior; it does not finalize the identity decision. Run identity.ps1 with a fresh ignored output directory, under the same bounded-job supervision as run.ps1 when unattended.

Guard regression tests (no private fixtures required):

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File oracle/test_run.ps1
```

These reject existing output directories and mismatched original/staged hashes before Office opens. The worker verifies the copied bytes actually opened, not only the supplied source. Missing Office is detected through real COM activation; all worker failures/timeouts produce failed metadata and a nonzero invocation. Timeout range is 1–3600 seconds. The parent requires worker metadata before reporting completion. Read-only automated opening does not independently exclude silent Office repair. Re-export the approved frozen copy rather than a Downloads file that may have changed; supply a generated package's own explicit hash when validating round trips.

The inventory CLI refuses the input path, symlink aliases and every existing output artifact before writing. Output creation is exclusive, so an artifact appearing after the initial check is also protected. Use a new JSON path for every run; no overwrite option is provided.
