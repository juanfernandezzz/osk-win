# Performance validation follow-up — osk-win 0.3.0

Date: 2026-11-16. Original baseline: `b13d7f0ca83dfcae8057bb47905c8bf083275612`
(tag `v0.3.0`). Application revision examined:
`7d01babf4e0f02775fd7826846519af9ac5f9aec`.

**Status: partial follow-up, not a completed second full-source review or a new
benchmark run.** This document adds validation findings, not application changes.
The execution environment is unavailable; the GitHub connection remains usable.
No new Rust build, test run, desktop interaction or audio measurement was possible.

## Installed tools and concrete use

The following skills were installed in ChatGPT, appear in its installed skill
catalog, and their installed SKILL.md contents were read:

| Skill | Source | Use in this follow-up |
|---|---|---|
| find-skills | [Vercel](https://github.com/vercel-labs/skills/tree/main/skills/find-skills) | Discovery and installation workflow completed before this follow-up; no additional skill selected here. |
| benchmark-checklist | [Cursor](https://github.com/cursor/plugins/blob/main/pstack/skills/benchmark-checklist/SKILL.md) | Re-read both measurement scripts, the Rust timed region, raw results and claims against all seven measurement questions. |
| property-based-testing | [Trail of Bits](https://github.com/trailofbits/skills/tree/main/plugins/property-based-testing/skills/property-based-testing) | Reviewed the new glyph/surface oracle tests and timer coverage using the installed reviewing reference; identified missing state sequences and measurable-work assertions. |

The two complementary contributions are measurement validity and independent
equivalence testing. They supplement the Rust optimization rules without changing
dependency, allocator, model or CPU-target choices. No new testing dependency was
introduced. These are ChatGPT installations; local CLI installation is not claimed.
The separately delivered combined skill remains outside this repository and
contains no find-skills instructions or link.

## Scope of this follow-up

Re-read in full: both Python measurement tools, the existing report, both raw
JSON datasets, application main and the root/application Cargo manifests.
Re-read changed hunks in render/ui, the glyph cache and its two tests, surface
creation/presentation, path operations, and relevant surrounding code for the
render benchmark, cache construction/invalidation, waveform, timer scheduling,
recording ownership and resampling. This is targeted inspection; it must not be
described as a second full read of every source file. The first-pass inventory
and claims remain in [the original report](performance-0.3.0.md).

## Benchmark checklist verdict

| Question | Evidence | Verdict / missing evidence |
|---|---|---|
| Limiter identified? | No recorded CPU/allocation profile. Timed region spans measurement, paint and pixel flush. | Inconclusive for speedup and attribution. No specific bottleneck is established. |
| Equivalent production tuning? | Same recorded toolchain/profile and no-default-features variant on both sides. Default Cargo feature is dictation. | Comparable rendering variant, not the full shipping feature configuration. |
| Plausible physical limits? | Offscreen cache removes one DIB/DC pair and nominal 4 × width × height DIB pixels. Glyph storage adds memory. | Structural resource reduction supported. Whole-process peak deltas cannot all be assigned to DIB removal. |
| Errors and correct outputs? | Timing tool requires successful exit, requested frame count and matching PNG hashes. Memory tool requires successful exit, nonzero samples, output existence and frame message. | Memory tool does not hash/compare its own output and does not record executable hashes. Timing validation cannot retroactively prove correctness of separate memory runs. |
| Reproducible? | Five processes per side/scenario, alternating pair order. Recalculated medians/ranges match stored summaries. | Timing ranges overlap. Historical memory experiment also had compiler interference. No new repetition occurred. |
| End-to-end relevance? | Rust benchmark redraws an unchanged View after initial paint. It omits input dispatch, BitBlt, real recording and inference. | Warm rendering result only; no typing, first-paint, idle-CPU or dictation latency claim. |
| Work actually happens? | The timed loop calls measure_bar, paint and flush; the final image is written after the loop. Requested frame count is checked. | Confirms path and output for the fixed scene, but not correctness of every intermediate state or avoided work on cache hits. |

### New interpretation: distinguish cache construction from cache reuse

`render::run` paints once before calling `bench_frames`. The loop then uses the
same View and KeyCache. Consequently, the measurements largely exclude cache
construction, where the removed native DIB creation and cache flush occur.
They also do not exercise theme/layout/modifier/size transitions, which can
rebuild the keyboard cache.

This explains a limitation of the experiment, not the cause of its noisy
numbers. The improvement could matter during rebuilds even if warm-frame time
barely changes; confirming that requires a separate rebuild scenario and an
end-to-end measurement. A profiler is still needed before naming the limiter.

### Recalculation of the recorded data

A fresh JavaScript calculation over the committed JSON confirmed:

- Four timing scenarios, five samples per side, exactly the reported medians
  and ranges, and one common image hash across each scenario's ten samples.
- 234 distinct argument combinations in the matrix. This checks the recorded
  dataset; it is not a rerun of the image comparisons.
- Twenty resource records: five per side at each of two sizes. All records have
  nonzero sample counts. The reported resource medians match the raw records.
- Recorded executable sizes differ by +5,120 bytes. Recorded hashes identify
  the timing binaries; this follow-up did not have their local bytes.

Additional memory dispersion, in bytes:

| Size | Metric | Baseline min–max | Candidate min–max |
|---|---|---:|---:|
| 800×300 | Private bytes | 16,662,528–16,826,368 | 15,626,240–16,838,656 |
| 800×300 | Working set | 21,561,344–21,770,240 | 20,832,256–22,011,904 |
| 1440×540 | Private bytes | 26,185,728–29,589,504 | 22,601,728–26,628,096 |
| 1440×540 | Working set | 30,683,136–34,107,392 | 27,787,264–31,072,256 |

The candidate's 800×300 sampled peak can exceed baseline despite its lower
median. The two GDI counts are consistently 4 and 2 in their respective groups.
Do not turn these medians into a guaranteed memory saving for every run.

## Equivalence-test review

These are test-coverage findings, not demonstrated runtime regressions.
Severity describes the consequence of relying on incomplete evidence.

| Severity | Existing evidence | Gap and strongest next check |
|---|---|---|
| MEDIUM | Glyph cache compares bitmap and used metrics with direct fontdue rasterization over chosen Unicode characters and sizes, drawing twice. | Keep this independent cache oracle, but revisit earlier keys after churn and explicitly exercise byte-limit and entry-limit eviction, oversize bypass and resizing sequences. Current cases do not cover the full input domain. |
| LOW | Cache oracle checks output equality and storage bounds. | An implementation that always rasterizes could still pass. Add a test-only rasterization counter or allocation/profile evidence to prove a hit avoids work; never add production hot-path instrumentation for this check. |
| MEDIUM | Surface test compares offscreen/presented Pixmap bytes; the historical matrix compares original/candidate PNGs. | Add sequences in one process: theme/layout/modifiers/size/DPI changes, reused cache versus fresh cache, nested clipping and save/restore. The surface test alone shares the same painter and cannot establish all painting semantics. |
| MEDIUM | Timer test asserts no deadline at rest. Diff keeps the same deadline sources, minimum, saturation and 1 ms floor. | Test simultaneous/past/future deadlines and each source under a controlled clock, comparing the fold to the original collected minimum. Static equivalence reasoning is not a timer test run. |
| MEDIUM | Waveform calculations moved from generic View construction to the paint snapshot; resampling remains unchanged and raw audio is dropped after it. | Compile and test the default feature configuration, then compare waveform and transcription outcomes. Rendering-only tests did not compile these changed audio-feature branches. |

The existing cache oracle is not tautological merely because fontdue is also
used internally: direct rasterization is the reference for the new caching
layer. Exact bitmap equality is appropriate for this contract. No critical
tautology or vacuous filter was identified in the two new tests.

## Concrete remaining work, in execution order

1. Restore a working Windows execution environment and build baseline/candidate
   with identical locked dependencies and the default dictation feature.
   Resolve libclang for the Windows ABI; do not disable assertions or substitute
   incompatible pre-generated bindings.
2. Add the focused equivalence checks above, then run the existing tests and
   image matrix. Include tiny positive canvases, fractional scales and repeated
   state transitions; the current matrix starts at much larger sizes.
3. Strengthen the memory harness to compare each run's output and record binary
   identities, per-call sampling errors and sample counts. Keep the historic
   JSON unchanged; label a new run separately.
4. Profile separately from timing. Measure cold creation/rebuild and warm repaint
   separately, then the native window path and real dictation. Keep unrelated
   compilation out of the experiment.
5. Only then test additional allocation candidates, individually: PathBuilder
   buffer reuse, audio ownership at 16 kHz and completed RMS-window reuse.
   Require pixel/audio equivalence and resource improvement before accepting.
   No new improvement from these candidates is claimed here.

A path-buffer reuse experiment must preserve empty/invalid path behavior,
fill/stroke/clip consumption and current-point state. An RMS reuse experiment
must preserve the partial-window boundary, oldest/newest order and history-based
gain. The cost of being wrong is changed pixels, timing of key events or audio
output; these are functional regressions, regardless of any speed gain.

## Publication boundary

This follow-up changes documentation only. The original release tag and
application source are untouched. The redo remains incomplete until the native
checks can run; neither the installed skills nor this audit substitutes for them.
