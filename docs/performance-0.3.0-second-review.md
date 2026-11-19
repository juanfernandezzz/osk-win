# Windows keyboard performance and feature revision

The original comparison starts at `v0.3.0` (`b13d7f0`). The first published
optimization is `7d01bab`. Its documentation follow-up is `28e795e`.
The user subsequently requested removal of voice features 21–27 and 31 and
a clickable version below Scroll Lock. This changes the product and image.
The intermediate path-buffer experiment kept voice and the original image.
Historical measurements remain in [the first report](performance-0.3.0.md).

## Source review and builds

Reviewed the original core controller, events, interaction, key definitions,
layouts, metrics, modifier logic, typing and self-tests. The Windows review
covered the painter, Cairo adapter, UI event loop, injection backend, key
translation, input helper, verifier, updater, CLI, logging and the original
dictation code. Build scripts, manifests, installer, font, certificate, tests,
measurement tools and prior evidence were included in the inventory. The
font and certificate are binary assets. No third-party source audit is implied.

The Windows builds used x86_64-pc-windows-msvc, rustc 1.99.1, the locked
dependencies, release fat LTO and one codegen unit. Baseline and first default
builds used native libclang 18 and the actual Windows bindings. The earlier
ABI assertion failure was resolved without patching the bindings. The product
revision prunes 102 of 178 locked packages and retains all remaining versions.
The quiet startup check and explicit version check use the same release feed,
version comparison, confirmation dialog and installer selection.

## Isolated rendering experiment

A separate ETW CPU profile on an i5-4210U produced 12,288 on-CPU samples.
The PDB resolved all 2,744 distinct application addresses. Fill appeared
in 5,318 stacks and stroke in 1,993. Inclusive stacks can overlap. Compilation
overlapped profiling, so these samples help locate work and do not measure
elapsed speedup.

Reusing tiny-skia path storage after fill, stroke and clip reduced Rust
allocation requests on the instrumented rendering thread:

| Work per frame | First optimization | Path reuse |
|---|---:|---:|
| 800×300, rebuild | 2,095 | 775 |
| 800×300, hot | 391 | 129 |
| 1440×540, rebuild | 2,205 | 885 |
| 1440×540, hot | 392 | 130 |

Five alternating fresh processes per variant reproduced every rebuild and hot
count in the table. Raw counts are in [allocation-runs.json](performance-second-review/allocation-runs.json).
These are diagnostic calls to allocate, zero-allocate or reallocate. They
exclude other threads, native allocations and retained capacity. The path
test covers discarded geometry, empty operations, clips and pixel output.
Before the product revision, all 234 sequence images matched the original
with default features. Additional tests cover byte and entry eviction, hits,
oversize glyphs, revisits, nested clips, tiny surfaces, native BitBlt and
deadlines against the original minimum with a controlled clock.

For voice, the original Windows build transcribed the public JFK WAV with a
real model. The 16 kHz Vec transfer only ran on synthetic exact-capacity
buffers. A reconstructed quiet clip differed in capitalization on both
compared variants. The original fixture was unavailable. Microphone capture
was not exercised. The user subsequently removed the entire voice path,
including capture, model loading, inference and voice diagnostics.

## Reproduction and scope

Use release binaries from the three matching-feature revisions with
`tools/perf_three_way.py` to measure hot painting and validate identical
decoded frames. Use `tools/perf_memory.py` separately for sampled private
bytes, working set and GDI. It records binary identities, decoding errors,
samples and per-call failures. The Python tests inject missing work, different
pixels, invalid PNGs and Windows API failures. The cache probe measures first
frame, forced rebuild and hot painting. Its second size inherits a warm font.
These probes do not time a full desktop click-to-character interaction.

The installed discovery skill verified the benchmark and property-testing
selections. The benchmark checklist led to independent CPU profiling and work,
image and binary validation. Property-testing guidance led to bounded state
sequences and independent reference oracles without new Rust dependencies.

The distribution assets matched the original byte for byte (font SHA-256
0e9698f561568a0d83ce644a495d228a6f83d8d0404906223852877438b171c2,
certificate 877731dc5ce458ae8477bbc18b7fb537fa691432f010c82d3b2758ec210c73d4).
For the older revisions, build the release test executable in an archived copy:
include the portable cache probe from render.rs as a test module, omit the
version-placement test and initialize the older View's dict_state to Idle
and levels to an empty Vec. To reproduce the old recording-state sequence,
set Recording and alternate fake_wave_levels(WAVE_BARS) with zero levels each
step. The timing probe always uses the idle voice state.

To reproduce the intermediate shipping code, start from `7d01bab` and take
`osk-win/src/cairo.rs` from `98325f3`. That file is byte-for-byte identical
to the intermediate source used here. For the final product, use `98325f3`
in full. The table below identifies the exact default executables measured.

## Windows results, 2026-11-19

At equal functionality the change with the clearest isolated effect was
path reuse: 262 fewer Rust allocation requests per hot frame at both sizes.
Frame time varied by workload. With the pointer over a key at 1440 × 540,
the median was 3.94 ms in the first optimization and 4.54 ms with path
reuse. With the menu open, the corresponding medians were 8.53 and 6.62 ms.
The ranges overlap in both comparisons. The measurements establish fewer
allocations and lower sampled memory against the original. They establish
neither a general frame-time improvement nor a reduction in typing latency.

In the separate product comparison, removing voice and adding the version
reduced the executable by 1,369,600 bytes against the intermediate build.
Median private bytes fell at both sizes. CPU time for 1,000 menu redraws
had slightly higher medians in the final product at both sizes, with
overlapping ranges.

Each scenario used five fresh processes per variant with rotating order. The tables show the median and the minimum–maximum range across those five process results. Cache phases report per-process frame medians. The 800 × 300 first frame starts with a cold process font. The 1440 × 540 first frame reuses that font. Measurements started after the other Rust compilation finished. The OS scheduler and other desktop activity were uncontrolled.

| Stage (ms) | Original | First | Path reuse | Voice removed + version |
|---|---:|---:|---:|---:|
| 800 × 300, first | 55.78 [46.62–72.88] | 51.89 [46.30–81.18] | 70.64 [49.01–176.36] | 50.71 [49.21–55.04] |
| 800 × 300, rebuild | 16.55 [14.68–22.11] | 14.82 [13.31–34.80] | 15.99 [12.88–23.39] | 15.32 [12.88–18.10] |
| 800 × 300, hot | 2.62 [2.42–2.79] | 2.42 [2.27–4.47] | 2.33 [2.18–4.43] | 2.33 [2.13–4.48] |
| 1440 × 540, first | 61.91 [35.42–134.87] | 68.04 [59.37–107.15] | 65.95 [37.39–91.32] | 62.78 [46.36–239.31] |
| 1440 × 540, rebuild | 37.56 [33.17–41.32] | 31.52 [30.02–48.67] | 32.46 [28.97–34.35] | 36.93 [29.34–45.63] |
| 1440 × 540, hot | 7.41 [5.99–12.00] | 5.62 [5.51–10.70] | 6.77 [5.40–6.92] | 6.30 [5.38–9.03] |

| Hot production scenario (ms/frame) | Original | First | Path reuse |
|---|---:|---:|---:|
| hover_800 | 1.09 [1.08–2.12] | 1.07 [1.07–1.27] | 1.06 [1.06–1.28] |
| menu_800 | 2.49 [2.40–2.73] | 2.27 [2.22–3.96] | 2.23 [2.20–4.28] |
| pressed_800 | 1.23 [1.08–1.80] | 1.07 [1.06–1.24] | 1.06 [1.05–1.24] |
| hover_1440 | 4.40 [4.10–4.85] | 3.94 [3.48–4.58] | 4.54 [4.33–7.39] |
| menu_1440 | 7.11 [6.38–14.59] | 8.53 [6.31–13.03] | 6.62 [6.46–10.69] |

| Sampled peak (original vs path reuse) | Original | Path reuse |
|---|---:|---:|
| 800x300 gdi_objects | 4.00 [4.00–4.00] | 2.00 [2.00–2.00] |
| 800x300 private_bytes | 17,608,704.00 [17,465,344.00–18,399,232.00] | 16,355,328.00 [16,162,816.00–17,293,312.00] |
| 800x300 working_set | 23,060,480.00 [22,847,488.00–23,748,608.00] | 21,942,272.00 [21,880,832.00–23,007,232.00] |
| 1440x540 gdi_objects | 4.00 [4.00–4.00] | 2.00 [2.00–2.00] |
| 1440x540 private_bytes | 27,344,896.00 [27,176,960.00–29,007,872.00] | 24,154,112.00 [23,445,504.00–24,297,472.00] |
| 1440x540 working_set | 32,198,656.00 [31,985,664.00–33,333,248.00] | 29,007,872.00 [28,954,624.00–29,753,344.00] |

Each of the 75 hot timing runs validated the decoded PNG written after its 500 redraws and checked the reported work count. The separate cache probe also compared the final image after each batch with the first frame. The 20 equal-work memory runs checked their image and the 1,000-frame count. Sampling errors, including failures after output and exit races, totaled 6 in the equal-work runs and 10 in the feature-change runs.

An earlier equal-work series stopped on its first pair after GetGuiResources returned error 87 before the candidate process signalled exit. Its 1,000 frames and PNG were valid. The partial series is preserved in [memory-first-attempt.json](performance-second-review/memory-first-attempt.json) and excluded from these summaries. The final harness preserves both observations in a pair when a sampling failure during the workload requires a retry. Both final experiments obtained five valid pairs per size on their first attempts. All 16 recorded errors occurred after the CLI had already reported writing its output.

A second preliminary series retained [three unsuccessful pairs](performance-second-review/memory-strict-retries.json). Error 87 occurred in each process near exit, but the original sampler could not establish whether drawing had finished. The final sampler records the byte length of the diagnostic log immediately before querying GDI. It records error 87 as post-render only if a positive GDI peak was observed and the complete final output line was already present before the call. The CLI emits that line after its renderer returns and drops its GDI surfaces. All errors remain in the reports.

Private bytes are sampled process peaks. CPU is total user plus kernel time for the entire process, including initialization, the 1,000 redraws and writing the PNG. The feature revision draws a different screen. Executable size includes linked code and data and excludes the separately downloaded model.

| Default exe | Bytes | SHA-256 |
|---|---:|---|
| baseline-default | 3,219,968 | 2de371c9a4e9425c2d1917a8a07681da0065261976283d53b6729ab842354681 |
| first-default | 3,226,112 | ff3b770076390dcb262793609cd7330394fbdb86f9a9fd9800273c5cc5a08ab5 |
| candidate-default | 3,225,088 | 3dc2922d97c0a2135324b4ce7d7aafc15ab9645d206383e92042b0ea679c810f |
| scope | 1,855,488 | d15fa911f4e08d5c7235ac0e4cadea0644d361109d0434af3bb599bc03e93fb7 |

| Product change, 1,000 menu frames | Path reuse with voice | Final keyboard |
|---|---:|---:|
| 800x300 cpu_ms | 2,562.50 [2,359.38–2,687.50] | 2,781.25 [2,265.62–3,078.12] |
| 800x300 private_bytes | 16,736,256.00 [16,310,272.00–17,326,080.00] | 15,794,176.00 [15,540,224.00–15,925,248.00] |
| 800x300 gdi_objects | 2.00 [2.00–2.00] | 2.00 [2.00–2.00] |
| 1440x540 cpu_ms | 6,546.88 [6,312.50–6,984.38] | 6,625.00 [6,203.12–7,671.88] |
| 1440x540 private_bytes | 24,092,672.00 [23,969,792.00–26,480,640.00] | 22,749,184.00 [22,376,448.00–26,091,520.00] |
| 1440x540 gdi_objects | 2.00 [2.00–2.00] | 2.00 [2.00–2.00] |

The final release test suite passed 68 tests, with one explicit performance probe run separately. The Python measurement harness passed nine tests. A running installed instance prevented simultaneous live GUI testing.

Measured hot paint excludes input delivery, window presentation and model loading. The allocation experiment supports the narrower path-buffer improvement. Desktop typing latency still requires live validation.
