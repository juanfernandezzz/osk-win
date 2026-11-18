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
