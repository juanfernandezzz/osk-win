# Performance review of osk-win 0.3.0

Baseline: tag `v0.3.0`, commit `b13d7f0ca83dfcae8057bb47905c8bf083275612`.
Review and measurements: 2026-11-15/16, Windows x86-64, rustc 1.99.1
(`58cf8f9c3`, LLVM 23.1.1), existing Cargo.lock and release profile.
The version, dependency graph, enabled-by-default dictation feature, input
semantics and distribution CPU target are unchanged. No release tag was moved.

## Changes and attribution

| Path | Repeated work removed | Preservation checks |
|---|---|---|
| `osk-win/src/cairo.rs`, `render.rs` | Cached keyboard layers use a Pixmap without an unused GDI DIB/DC or RGBA-to-BGRA flush. | Offscreen/presented pixel oracle and baseline image matrix. |
| `osk-win/src/cairo.rs` | Reuse rasterized glyphs on the UI thread; bound retained bitmap lengths to 64 KiB and entries to 256. Exact character/float-size keys; color and simulated bold remain downstream. | Direct fontdue oracle across Unicode, fractional/large sizes, hits and eviction; bounded metadata for empty glyphs. |
| `osk-win/src/render.rs` | Borrow static labels with Cow, use static F1–F12 labels, iterate lines without a temporary Vec, use a rectangular path for zero-radius corners. | Baseline image matrix and existing modifier/layout tests. |
| `osk-win/src/ui.rs` | Fold timer deadlines directly instead of allocating a Vec. Read waveform levels only once per paint, never for geometry/hit testing. Drop source audio after resampling before inference. | Existing event/idle tests; audio changes retain the same RMS, gain, resampling and inference implementations. |

The offscreen change eliminates `4 * width * height` bytes of native bitmap
pixels per keyboard cache (937.5 KiB at 800×300, 3037.5 KiB at 1440×540), plus
one bitmap and one DC. The glyph cache adds bounded pixel storage **and** table,
metrics and allocator overhead; 64 KiB is not a total-heap bound. Oversized
glyphs are rendered without retention. No dependency was added.

## Timing evidence

Five fresh processes per executable and scenario, alternating baseline/candidate
order, 500 measured frames each after the program's initial render. Neither
compilation nor profiling ran during these timing runs. Other desktop processes
and CPU scheduling were not controlled. This is a local exploratory comparison,
not a statistical guarantee or a claim about typing latency.

| Scenario | Baseline median ms [range] | Candidate median ms [range] |
|---|---:|---:|
| Hover, 800×300 | 1.140 [1.046–2.070] | 1.177 [1.024–1.838] |
| Menu, 800×300 | 2.691 [2.511–4.916] | 2.475 [2.267–3.291] |
| Hover, 1440×540 | 5.105 [4.525–5.834] | 4.701 [4.262–6.174] |
| Pressed, 800×300 | 1.101 [1.046–1.129] | 1.042 [1.020–1.111] |

Each table median is the median of five per-process frame medians; ranges are
the smallest/largest of those medians. Three scenarios have lower medians and
one has a higher median. Variation overlaps substantially: **no general runtime
speedup is established**. The earlier changes without glyph caching also had
overlapping timings, so no independent percentage is attributed to each edit.
Less repeated rasterization/allocation is established by the code paths and
oracles, but a controlled profile would be needed for quantitative attribution.

The timed path includes bar measurement, drawing, pixel conversion and GdiFlush.
It excludes window BitBlt, input dispatch, idle behavior, first-paint latency,
helper IPC, audio and inference. Whole-process wall times in the raw data also
include startup and PNG output; they are not startup-to-first-paint measurements.

## Resource evidence

Separate Windows GetProcessMemoryInfo/GetGuiResources sampling every 10 ms,
five alternating runs per side and size, menu visible, 1000 frames per run.
The following values are medians of the sampled per-process peaks:

| Size | Metric | Baseline | Candidate |
|---|---|---:|---:|
| 800×300 | Private bytes | 16,695,296 | 15,896,576 |
| 800×300 | Working set | 21,614,592 | 21,049,344 |
| 800×300 | GDI objects | 4 | 2 |
| 1440×540 | Private bytes | 28,930,048 | 23,465,984 |
| 1440×540 | Working set | 32,460,800 | 27,869,184 |
| 1440×540 | GDI objects | 4 | 2 |

These are sampled peaks, not total allocations or guaranteed maxima. Compiler
activity overlapped part of this separate resource experiment; do not use its
durations as timing evidence. Allocation layout, paging and polling can vary.
Confidence is high for removal of the redundant native buffer and two GDI
objects, moderate for lower memory in these workloads, and unknown for the
complete desktop application's memory under real dictation.

Executable size: 1,840,128 → 1,845,248 bytes (+5,120 bytes, about 0.28%). This
change trades a small amount of code/cache storage for less repeated work and
native-buffer memory; it does not claim a smaller executable.

## Validation

- Release workspace tests without default features: **56 passed**, versus 54
  before the two new rendering tests.
- **234 exact PNG comparisons passed**: 3 layouts × 13 themes × 6 state/size
  pairs, including idle, hover, pressed, menu, modifiers, handle animation,
  small and asymmetric canvases. The four timing cases also matched exactly.
- `cargo clippy --release --locked --offline --workspace --no-default-features
  -- -W clippy::perf` completed. Only the existing `OSK_MARK` hex-grouping
  warning remained (shared by both binaries); no performance warning appeared.
- All-feature tests were attempted but **blocked before the application compiled**:
  libclang was unavailable. The dependency's pre-generated bindings failed
  Windows layout assertions (`_G_fpos_t`, `_G_fpos64_t`, `_IO_FILE`). CMake/MSVC
  discovery and long object paths were resolved using the installed VS tools,
  Ninja and a short temporary target directory, but that does not fix the ABI
  mismatch. The all-feature build/tests and real dictation remain unverified;
  neither dependencies nor feature defaults were changed to mask this failure.

The matrix is a finite set, not all combinations or interaction sequences. No
real microphone, Whisper model accuracy, elevated target application, installer,
window focus, physical-key integration or end-to-end input latency was exercised.
Tests do not prove every existing function for all inputs. A pixel mismatch,
changed injected event sequence, changed waveform/inference output, or a
repeatable resource/latency regression would falsify the corresponding claim;
revert the responsible isolated change if that occurs.

## Coverage and deliberately unchanged candidates

All Rust source files in the pinned tree were reviewed, including tests. Build
configuration and scripts were inspected for runtime-impacting settings; the
embedded font and certificate were treated as unchanged binary assets.

| Area | Review conclusion |
|---|---|
| `core/src/{controller,events,interaction,key,layout,lib,metrics,mods,typekey}.rs` | Static layouts, compact state and bounded event sequences already fit the workload. Preserve ordering, latch/lock, repeats and modifier releases. |
| `core/src/selftest.rs`, `core/tests/*` | Diagnostic-only name formatting is cold; changing it would not improve ordinary keyboard use. |
| `osk-win/src/{cairo,render,ui}.rs` | Changes above; do not introduce dirty-region or mask-sharing complexity without profiles and nested-clip/state tests. |
| `osk-win/src/dictate.rs` | Model caching and existing decode tuning preserved. Incremental RMS windows, owned 16 kHz buffer reuse and PCM normalization are future measured experiments; quality cannot be assumed from render tests. |
| `osk-win/src/{backend,vk}.rs`, `src/bin/osk-win-input.rs` | Preserve stack-based input records, mappings, named-pipe protocol and helper behavior. No batching that changes key ordering. |
| `osk-win/src/{main,log,update}.rs` | CLI and event lifecycle retained. No added idle polling or logging in hot paths. |
| `oskwin-verify/src/main.rs` | Polling clones could be reduced, but this is a diagnostic executable, not the keyboard's normal runtime. |
| Cargo manifests/lock/config, `osk-win/build.rs`, installer and five shell scripts | No feature removal, CPU compatibility change, dependency update or blind allocator/profile switch. Installer and signing workflow unchanged. |

## Reproduction

Build the pinned baseline and this revision with the same toolchain and flags,
preserving the baseline executable before building the candidate:

```powershell
cargo test --release --locked --offline --workspace --no-default-features
cargo build --release --locked --offline -p osk-win --no-default-features
python tools/perf_compare.py --baseline PATH_TO_BASELINE --candidate PATH_TO_CANDIDATE --output OUTPUT_DIR --runs 5 --frames 500 --matrix
python tools/perf_memory.py PATH_TO_BASELINE PATH_TO_CANDIDATE OUTPUT_DIR
```

The no-default-features executable is the variant measured here; default
dictation support remains in Cargo.toml. Raw inputs, executable SHA-256 hashes,
samples and image hashes are in [timing.json](performance-0.3.0/timing.json) and
[memory.json](performance-0.3.0/memory.json). Python tools use only the standard
library and launch the headless render path without typing or recording audio.

## Criteria used

The relevant Microsoft checklist and internal performance/application/API/test
sections were evaluated together with the Pixel copy and its ownership rules.
Deduplicated criteria include hot-path measurement, capacity/reuse, indirection,
boxed immutable data, hashing, telemetry cost, no empty cycles, async state and
yielding, allocator/CPU tradeoffs, cheap shared handles, mockable system calls and
independent test oracles. Async-specific changes are not justified for this
Win32 event loop. Naming/style/security-only rules are outside this review.

Microsoft's general mimalloc recommendation conflicts with the rulebook's
measure-first allocator guidance; no allocator change is made without evidence.
Pixel's project-specific exclusions and 24-byte Copy heuristic are not universal
rules for this repository. Cache-only statics are permitted by Microsoft's
performance exception. Compile-time improvements are not runtime improvements.

Sources:
- [Microsoft checklist and internal guidelines](https://microsoft.github.io/rust-guidelines/guidelines/checklist/index.html)
- [Rust performance rulebook](https://github.com/joshuadavidthomas/agent-skills/blob/main/rust/performance.md)
- [Profiling and benchmarks](https://github.com/joshuadavidthomas/agent-skills/blob/main/rust/references/profiling-and-benchmarking.md)
- [Allocation and data structures](https://github.com/joshuadavidthomas/agent-skills/blob/main/rust/references/allocation-and-data-structures.md)
- [Pixel guidelines](https://github.com/Pixel-CLI/pixel/tree/main/.agents/skills/rust-guidelines)
- [Find-skills](https://github.com/vercel-labs/skills/tree/main/skills/find-skills)
- [Benchmark checklist](https://github.com/cursor/plugins/blob/main/pstack/skills/benchmark-checklist/SKILL.md): physical/Amdahl limits, valid work counts, comparable tuning, limiter attribution and alternating repeated trials.
- [Property-based testing](https://github.com/trailofbits/skills/tree/main/plugins/property-based-testing/skills/property-based-testing): independent equivalence oracles, nonvacuous cases, boundaries and reproducibility. Used deterministic cases, without a new dependency.

The external skill texts were read and applied. Their persistent installation
was blocked by the environment's write/network restrictions and unavailable
offline `skills` npm package; installation is not claimed. The standalone
combined skill is delivered separately and is intentionally absent from this
repository.
