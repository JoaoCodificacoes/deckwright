# Benchmarking

Run this stdlib Python harness **inside WSL on its Linux filesystem** first. Native Windows, mounted NTFS and UNC are separate profiles; no native Windows performance claim follows from WSL results. Python 3.10+ is required. No dependencies or downloads occur in timed commands.

```sh
python3 -m unittest discover -s bench -v
python3 bench/run.py --result-dir build/bench/inspect-reference \
  --input fixtures/private/reference.pptx --cache-state warm-uncontrolled \
  --filesystem-label WSL2-ext4 -- target/release/dw inspect fixtures/private/reference.pptx --json
python3 bench/run.py --result-dir build/bench/roundtrip-reference \
  --input fixtures/private/reference.pptx --cache-state warm-uncontrolled \
  --filesystem-label WSL2-ext4 -- target/release/dw roundtrip fixtures/private/reference.pptx --output '{output}'
```

Use the release binary after compilation completes, with pinned dependencies. The runner invokes argv directly with `shell=False`. Only a whole argument `{output}` becomes a per-run `artifact.pptx`; quoting or shell metacharacters are never evaluated. Result directories must be new. Every warmup/sample stores stdout, stderr and its artifact independently. Run failure or timeout prevents a success summary and yields exit 1. Disk capacity must cover 55 outputs per profile; retain or delete only the explicitly chosen result directory after evidence review. Inputs stay immutable and source/private assets must never enter Git.

Defaults are five warmups and fifty measured process launches. Wall time covers Popen through process exit, including loading, complete JSON serialization and file output performed by that process. Stdout is saved to disk, so it is a real serialized output workload. Median uses the ordinary sample median; p95 uses nearest rank (rank 48 for 50 samples). Timer polling has up to approximately 1 ms detection overhead, so this CLI driver cannot qualify the 5 ms in-process budgets. Child `wait4().ru_maxrss` supplies per-launch peak RSS (KiB converted to bytes on Linux, native bytes on macOS); it is not the harness's RSS or cumulative `RUSAGE_CHILDREN`. Linux may include waited descendants and inherited pre-exec memory, so use a direct CLI binary rather than a wrapper. Unsupported platforms explicitly report null/unknown memory. Windows process-tree timeout cleanup and RSS are not supported by this initial runner: qualify the direct native CLI separately before Windows support claims.

Manifests retain all raw samples, return codes, output sizes, argv, host OS/CPU/RAM/Rust/Python/filesystem data, input hash/bytes and ZIP byte counts. Stage breakdown is explicitly unavailable until application instrumentation exists; never infer it by subtracting unrelated runs. Cache state and filesystem labels are mandatory. Process restart is not cold filesystem cache. Do not flush caches implicitly; record a controlled cold-cache procedure separately if used. Record host power mode and competing activity for each qualified run. `bench/baseline-machine.json` freezes the initial host description, not an approved latency baseline.

For final gates reserve an idle fixed host: pause Cargo builds, PowerPoint exports and competing workers. Compilation, oracle export, transport and visual review have separate measurements. Shared CI and preparation smoke runs are advisory. Compare only matching profiles/configurations and pin the measured Git SHA. Dedicated gates fail for either budget violations or reproducible regression exceeding **both 10% and 2 ms** against a reviewed baseline. Budget changes need recorded rationale/review.

| Operation, 25-slide reference | p95 budget |
|---|---:|
| Complete structural/timing inventory JSON | 250 ms |
| Full authored build to closed PPTX | 1,000 ms |
| One-slide warm edit and write | 250 ms |
| Warm state evaluation + serialization | 5 ms |
| Warm adjacent Morph diff + serialization | 5 ms |
| Complete open-deck structural/timing checks | 50 ms |
| CLI state including startup/load | 300 ms |
| Peak process RSS for open/build/check | 128 MiB |

Roundtrip is preparation evidence, **not** full authored build. Missing CLI operations/in-process session measurements and internal stages remain pending. On missed budgets profile ZIP compression, XML parsing, resolution, timing compilation, JSON encoding and writes; preserve all correctness gates.

Generate neutral public inputs or private reference stress decks:

```sh
python3 bench/generate.py --output fixtures/generated/bench/neutral.pptx
python3 bench/generate.py --source fixtures/generated/bench/neutral.pptx --output build/bench/neutral-10x.pptx
python3 bench/generate.py --source fixtures/private/reference.pptx --output build/bench/reference-shared.pptx --factor 10
python3 bench/generate.py --source fixtures/private/reference.pptx --output build/bench/reference-media.pptx --factor 10 --unique-media
```

The source must be a valid OPC presentation with internal targets resolvable without external fetching. The generator copies the entire `ppt/` dependency graph into replica namespaces, remaps every internal relationship, appends presentation slide relationships/IDs and content-type overrides. Shape/timing IDs retain their slide-local scope. Notes and their backlinks are **cloned and remapped per replica**, not shared between cloned slides. Original XML/media parts are unchanged. Shared mode preserves one original media set and excludes repeated media bytes from the structural denominator. The growing-media profile clones media too; valid PNG ancillary `tEXt` chunks with replica identifiers are inserted before IEND with correct lengths/CRC, retaining compressed image pixels. Non-PNG media (including MP4) retains original bytes, explicitly counted as repeated hashes. Thus this is a distinct growing-PNG-media workload, not a claim that all media hashes are unique.

Validate XML, internal targets, relationship IDs, presentation IDs, slide-local shape IDs and slide relationship bindings after generation. Repeated inputs produce deterministic ZIP metadata/order. This is structural stress generation, not authored reconstruction; it does not prove Office playback or visual equivalence. Open/roundtrip checks and the oracle remain required. Reference structural mode targets no more than 15x runtime and 12x memory growth at 10x logical slide/object/effect workload. Compare per-profile total, XML and media bytes and object counts; unique-media growth has a separate denominator. No reference-derived fixture or media is committed.
