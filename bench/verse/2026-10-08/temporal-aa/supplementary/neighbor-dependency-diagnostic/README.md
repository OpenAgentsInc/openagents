The neighbor-dependency attempt still fails the live fast-head visual gate. Frames 471 and 472 show detached TAA-only head contours; the matching off views lack them. Native crops, the matching full images, source inventories, raw measurements, all eight native test logs, and GPU receipts are retained.

This 540-frame High run uses source `290c1590726189dbdabb5693125cfa0fbca8a779` and a dev binary with optimized dependencies. It ran under a GPU lease with overlapping CPU work. It establishes no timing gate and does not replace the top-level unresolved attempt. The eight passing native fixtures use source `340df670f658276c6074d33ec80ccdfc66dac7c3`; they did not exercise complete Photo::encode orchestration.

See [verification.json](verification.json), [matched head sheet](audit/head-470-472.png), and [source image inventory](source-images.json).
