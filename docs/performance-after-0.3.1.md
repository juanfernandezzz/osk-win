# Windows rendering performance after 0.3.1

The renderer now reuses the toolbar pixels when the layout button, theme button, minimize button, close button and menu state have the same appearance. A miss paints the toolbar onto the full destination and saves its top rows before drawing key overlays. It also shares a graphics context between measurement and painting, clips glyph iteration to visible pixels, and reads the hold timer only for the active key. The production Rust diff adds 116 lines and removes 127. The Windows binaries are 1,862,656 and 1,863,680 bytes respectively.

At 800×300 the live test used 23.7% less CPU time and finished its verified transitions 18.0% sooner by median. The sampled private-byte peak increased by 180 KiB at 800×300 and 496 KiB at 1440×540. The 1440-wide rebuild median increased about 6.1%. The spread across five runs is included below.

## Compared builds

The reference is v0.3.1 at `85d5f2d7b6e5e2d0eca6eec0a8d77f8f814c4ad4`. Both sides were built on the same Windows machine with the same locked dependencies, default features, release profile and `x86_64-pc-windows-msvc` target. The compiler was rustc 1.99.1 (58cf8f9c3 2026-10-13). SHA-256 identities and each run are in [the measurement data](performance-after-0.3.1.json). For the original 0.3.0 and its first optimization see the [earlier comparison](performance-0.3.0.md) and [second review](performance-0.3.0-second-review.md). The old 0.3.0 measurements are historical. All tables below are new and compare the same 0.3.1 feature set.

## Window repaint on Windows

The harness sent alternating mouse-move messages over the A key and a neutral area in a visible 800×300 test window. The window opened under the existing cursor. The driver checked window ownership at the sample point and waited for the expected key color after each forced repaint. It invalidated the client and sent a synchronous WM_PAINT. GDI GetPixel verified a changing key pixel against the reference render after every frame, including the native blit. Five alternating pairs of fresh processes drew 300 verified transitions each. CPU is Windows user plus kernel time for the keyboard process over the loop. Wall time also includes message delivery and the external pixel check. Table entries are the median [minimum, maximum] across the five processes.

| Scenario | Metric | 0.3.1 | Candidate | Change of medians |
|---|---|---:|---:|---:|
| 800×300 live | CPU ms / 300 changes | 921.875 [812.5, 1,078.125] | 703.125 [578.125, 828.125] | -23.7% |
| 800×300 live | wall ms / 300 changes | 1,155.066 [1,144.795, 1,557.058] | 947.311 [897.324, 1,181.746] | -18.0% |

## Warm render with identical output

Each process rendered 500 frames with one fixed interaction and exported a decoded PNG. Frame counts, exit codes and pixels were checked on every run. Each of five pairs reversed the previous execution order. The CLI includes the graphics flush. It excludes the window's final BitBlt and key delivery.

| Scenario | Metric | 0.3.1 | Candidate | Change of medians |
|---|---|---:|---:|---:|
| 1440x540 hover | CPU ms / 500 frames | 2,343.75 [1,953.125, 2,437.5] | 1,343.75 [1,281.25, 1,437.5] | -42.7% |
| 1440x540 hover | median frame ms | 4.561 [3.856, 5.053] | 2.448 [2.426, 2.763] | -46.3% |
| 1440x540 menu | CPU ms / 500 frames | 3,781.25 [3,390.625, 3,843.75] | 2,640.625 [2,593.75, 2,765.625] | -30.2% |
| 1440x540 menu | median frame ms | 7.691 [6.964, 9.474] | 5.375 [5.219, 7.474] | -30.1% |
| 800x300 hover | CPU ms / 500 frames | 609.375 [562.5, 734.375] | 484.375 [468.75, 515.625] | -20.5% |
| 800x300 hover | median frame ms | 1.196 [1.04, 1.703] | 0.768 [0.755, 0.775] | -35.8% |
| 800x300 menu | CPU ms / 500 frames | 1,312.5 [1,218.75, 1,390.625] | 1,109.375 [937.5, 1,125] | -15.5% |
| 800x300 menu | median frame ms | 2.515 [2.332, 2.836] | 1.969 [1.923, 2.088] | -21.7% |
| 800x300 pressed | CPU ms / 500 frames | 656.25 [640.625, 750] | 484.375 [453.125, 562.5] | -26.2% |
| 800x300 pressed | median frame ms | 1.059 [1.05, 1.408] | 0.765 [0.744, 0.86] | -27.8% |

## First frame and cache reconstruction

The release test probe ran in five fresh processes per variant and alternated order. Each process timed one first frame, 80 forced rebuilds and 500 warm repaints at each size, with the theme menu open. Each process checked its rendered output. The first frame at the second size shares process-level resources initialized at the first size.

| Scenario | Metric | 0.3.1 | Candidate | Change of medians |
|---|---|---:|---:|---:|
| 800 first | microseconds / frame | 51,772.1 [49,658.5, 68,273.3] | 52,197.1 [49,209.2, 99,484.3] | +0.8% |
| 800 hot | microseconds / frame | 2,453.2 [2,277.3, 2,826.85] | 1,955.35 [1,919.35, 2,016.7] | -20.3% |
| 800 rebuild | microseconds / frame | 15,648.65 [14,948.65, 23,416.95] | 15,592.85 [14,302.75, 21,787.65] | -0.4% |
| 1440 first | microseconds / frame | 70,533.4 [67,122.9, 96,059] | 66,268.2 [63,183.6, 67,385.8] | -6.0% |
| 1440 hot | microseconds / frame | 6,303.35 [6,036.95, 7,613.2] | 5,414.1 [5,210.4, 5,485.1] | -14.1% |
| 1440 rebuild | microseconds / frame | 33,035.8 [32,583.3, 41,455.75] | 35,042.25 [33,502, 40,018.35] | +6.1% |

## Allocation requests and memory

A separate diagnostic build counted requests to the Rust system allocator on the tested thread, including reallocations. The counted render call excludes conversion to PNG. Native allocations and other threads are outside the counter. Five alternating pairs were run separately from timing. Requested bytes are cumulative per frame.

| Scenario | Metric | 0.3.1 | Candidate | Change of medians |
|---|---|---:|---:|---:|
| 800 cold | requests / frame | 112,265 [112,265, 112,265] | 112,262 [112,262, 112,262] | -0.0% |
| 800 cold | bytes requested / frame | 60,490,235 [60,490,235, 60,490,235] | 60,340,088 [60,340,088, 60,340,088] | -0.2% |
| 800 hot | requests / frame | 129 [129, 129] | 110 [110, 110] | -14.7% |
| 800 hot | bytes requested / frame | 925,894 [925,894, 925,894] | 407,297 [407,297, 407,297] | -56.0% |
| 800 rebuild | requests / frame | 775 [775, 775] | 772 [772, 772] | -0.4% |
| 800 rebuild | bytes requested / frame | 21,324,647 [21,324,647, 21,324,647] | 21,174,500 [21,174,500, 21,174,500] | -0.7% |
| 1440 cold | requests / frame | 1,189 [1,189, 1,189] | 1,186 [1,186, 1,186] | -0.3% |
| 1440 cold | bytes requested / frame | 66,200,287 [66,200,287, 66,200,287] | 65,730,780 [65,730,780, 65,730,780] | -0.7% |
| 1440 hot | requests / frame | 130 [130, 130] | 111 [111, 111] | -14.6% |
| 1440 hot | bytes requested / frame | 2,546,242 [2,546,242, 2,546,242] | 950,255 [950,255, 950,255] | -62.7% |
| 1440 rebuild | requests / frame | 870 [870, 870] | 867 [867, 867] | -0.3% |
| 1440 rebuild | bytes requested / frame | 66,018,351 [66,018,351, 66,018,351] | 65,548,844 [65,548,844, 65,548,844] | -0.7% |

The memory harness sampled five pairs of 1,000-frame menu renders per size and validated every PNG. The sampling interval target was 10 ms. All 10 recorded errors were GetGuiResources error 87 after final workload output. Valid GDI samples preceded them in each affected process. There were 0 excluded pairs. Values are sampled process peaks. The toolbar cache retains one additional strip of up to one device row beyond the rounded-up toolbar height, bounded by the canvas.

| Scenario | Metric | 0.3.1 | Candidate | Change of medians |
|---|---|---:|---:|---:|
| 1440x540 | private bytes | 25,919,488 [22,540,288, 26,566,656] | 26,427,392 [23,515,136, 26,603,520] | +2.0% |
| 1440x540 | GDI objects | 2 [2, 2] | 2 [2, 2] | +0.0% |
| 800x300 | private bytes | 15,847,424 [15,622,144, 16,834,560] | 16,031,744 [15,499,264, 16,871,424] | +1.2% |
| 800x300 | GDI objects | 2 [2, 2] | 2 [2, 2] | +0.0% |

## Equivalence and limits

The candidate passed 73 release tests. The normally ignored cache probe was executed separately in all ten benchmark processes. The release suite exercised the existing input, updater, layout, clipping and rendering checks. Across builds, all 234 original state-sequence PNGs, 65 toolbar-transition PNGs and four clipped colored-text PNGs matched byte for byte. The two initial attempts to time the live window ended before the first accepted run because the sampled pixel had not changed or the new window was still obscured. The final harness waited for visible window ownership and observed every transition. The installed keyboard was closed normally for the test and its window and executable path were verified after relaunch. The toolbar sequence includes small positive canvases, fractional scales, menu toggles, hover changes, and a revisit with the same cache. A separate unit check covers equal-width source copies for taller, shorter and equal-height destinations, including partially transparent pixels.

An earlier experiment drew into a short canvas on misses. It changed the blue channel of a single pixel by one level at 14×10. The full-destination approach passed the same case. Measurements apply to the stated workloads on this machine. They do not establish typing latency, time spent idle, a change in background update speed or a percentage for a whole user session. Startup, networking and user interaction are outside the CLI loops.
