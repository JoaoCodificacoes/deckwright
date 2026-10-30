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

The source must be a valid OPC presentation with internal targets resolvable without external fetching. The generator copies slide and notes parts into replica namespaces, remaps their internal relationships, appends presentation slide relationships/IDs and content-type overrides. Shape/timing IDs retain their slide-local scope. Notes and their backlinks are **cloned and remapped per replica**, not shared between cloned slides. Original XML/media parts are unchanged. Cloned slides share the original registered slide masters, layouts and themes; notes masters also remain shared. No unregistered master copies are created. The presentation XML receives a narrow byte insertion, preserving original namespace declarations and prefix-valued `mc:Ignorable`/`Requires` attributes. Shared mode preserves one original media set and excludes repeated media bytes from the structural denominator. The growing-media profile clones media too; valid PNG ancillary `tEXt` chunks with replica identifiers are inserted before IEND with correct lengths/CRC, retaining compressed image pixels. Non-PNG media (including MP4) retains original bytes, explicitly counted as repeated hashes. Thus this is a distinct growing-PNG-media workload, not a claim that all media hashes are unique.

Validate XML, internal targets, relationship IDs, presentation IDs, slide-local shape IDs and slide relationship bindings after generation. Repeated inputs produce deterministic ZIP metadata/order. This is structural stress generation, not authored reconstruction; it does not prove Office playback or visual equivalence. Open/roundtrip checks and the oracle remain required. Reference structural mode targets no more than 15x runtime and 12x memory growth at 10x logical slide/object/effect workload. Compare per-profile total, XML and media bytes and object counts; unique-media growth has a separate denominator. No reference-derived fixture or media is committed.

## Wave 2 bounded structural checks

The library exposes `deckwright::check::check_path(path, Limits)` and
`check_package(&Package)`. Reports identify `dw.check/1`, import status,
error/warning severity, slide part, shape ID, message and repair hint.
Import failures (including missing internal relationship targets) produce an
error report with `imported: false`; structural rules do not run on failed
imports. `has_errors()` allows callers to choose an error exit status.

Implemented rules are duplicate slide-local shape IDs (error), missing encoded
`p:spTgt/@spid` targets (error), and duplicate explicit `!!` Morph names
within a slide (warning). Namespace lookalikes are excluded. Reports explicitly
state that inherited/rotated/group bounds, Morph pairing and timing playback
are unknown. These checks describe encoded package structure and do not
certify visual quality, accessibility, geometry or Office playback.

The runner accepts `--revision`, `--power-mode` and
`--competing-activity` so evidence records its measurement context.
Keep binary and harness SHA-256 values alongside the manifest when the harness
differs from the measured application revision.
## Reserved WSL measurement, 2026-10-26

A coordinator reservation kept the three task workers free of Cargo builds and
Office exports while six profiles ran serially, each with five warmups and fifty
samples. Windows power plan was Balanced
(`381b4222-f694-41f0-9685-ff5bb260df2e`); background OS activity and filesystem
cache remained uncontrolled. Inputs, output and the direct release binary were
on WSL2 ext4. These are local measurements, not native Windows qualification or
a regression claim against a reviewed prior baseline.

Measured production source was
`2e4a87a663360fd7b3ac9756f95ec990c720913c` (the structural check module was
present, but the measured commands were existing inspect/roundtrip paths).
Only test and documentation edits occurred during measurement. Each private
manifest stores the exact binary and harness hashes, input hash/bytes, complete
argv, host details, raw child measurements and output sizes. Private raw evidence
is under `build/bench/` and is excluded from Git.

The frozen authorized source SHA-256 was
`ee8cce03cf0201150fdd6f9c75f5d551538ed90d6ffae1c417e82b939e027e2b`.
The 10x shared-media input has 250 slides, 9,760 shape elements, 5,330 effect
preset nodes, 614 parts and 4,884,647 compressed bytes. The growing-PNG input
has the same logical slide workload, 1,244 parts and 39,412,786 compressed bytes;
691 of 700 media hashes are distinct. Non-PNG bytes remain shared by hash.

| Profile | Operation | Median ms | p95 ms | Peak child RSS MiB |
|---|---|---:|---:|---:|
| Source, 25 slides | inspect + complete JSON | 186.60 | 205.32 | 78.75 |
| Source, 25 slides | roundtrip | 247.51 | 293.81 | 46.73 |
| Shared media, 250 slides | inspect + complete JSON | 1680.03 | 2046.40 | 681.75 |
| Shared media, 250 slides | roundtrip | 796.10 | 920.22 | 294.27 |
| Growing PNG media, 250 slides | inspect + complete JSON | 2009.46 | 2262.22 | 732.82 |
| Growing PNG media, 250 slides | roundtrip | 2328.17 | 3204.46 | 377.20 |

The 25-slide complete inventory meets its 250 ms p95 and 128 MiB reference
budgets. Shared-media 10x inventory grows 9.97x in p95 latency and 8.66x in
peak RSS, meeting the separate 15x/12x structural growth gates. The actual
250-slide inventory still uses 681.75 MiB: eager XML trees and full inventory
serialization are material limits. The growing-PNG profile is reported
separately because its media byte denominator also grows. No budgets were
relaxed.

All 330 launches exited successfully without timeouts. Roundtrip results are
preparation evidence, not authored-build results. Check latency, warm state,
Morph diff and full authored build remain unqualified. Stage breakdown remains
unavailable; these measurements do not isolate XML parsing, JSON encoding or
compression. No reviewed baseline was available for regression qualification.
Strict OOXML namespace rules and full XML schema validation are also outside
the current structural checker.

## Query benchmark

`crates/deckwright/benches/query.rs` measures in-process slide-state evaluation and one adjacent Morph diff, each including `serde_json` serialization. Open or build time is reported separately and never mixed into samples. It uses std only and prints one `dw.bench.query/1` JSON object on stdout.

```sh
cargo bench -p deckwright --bench query
DW_BENCH_INPUT=/path/deck.pptx cargo bench -p deckwright --bench query
DW_BENCH_SAMPLES=10 cargo bench -p deckwright --bench query
```

Without `DW_BENCH_INPUT`, a synthetic 25-slide workload is built in memory. With it, the local deck is opened and the state slide (most timing effects) and Morph pair (most explicit matches) are measured. Five warmups run before the samples; `DW_BENCH_SAMPLES` overrides the default 50. Statistics use nearest-rank p95 and a 95% bootstrap interval (2000 resamples, fixed seed). Each operation carries a pass/fail verdict on p95 against the 5 ms budget.

Results from this benchmark are local evidence only. Dedicated gates are the same as above: fail on a budget violation, or on a reproducible regression exceeding both 10% and 2 ms against a frozen baseline. Shared CI results are advisory only.

## MCP transport overhead

`crates/dw-mcp/benches/transport.rs` compares one `state` request through `session::Server::handle_line` with the same query as `tools/call` `dw_state` through `McpServer::handle`, on one open handle. Both timings include request handling and response serialization; request lines are prebuilt. It builds a synthetic three-slide deck with a click fade on slide 2, uses std and `serde_json` only, and prints one `dw.bench.mcp/1` JSON object. Each path reports nearest-rank median and p95 with raw nanosecond samples, and `overhead_ms` is the MCP value minus the session value for each statistic.

```sh
cargo bench -p dw-mcp --bench transport
DW_BENCH_SAMPLES=10 cargo bench -p dw-mcp --bench transport
```

Five warmups run before the samples; `DW_BENCH_SAMPLES` overrides the default 50. The benchmark has no budget and no gate. Its numbers are local, advisory measurements from one machine and are not evidence of a regression on their own.
