# Actual ordinary native play at 120 Hz

The isolated matching-17a bundle from `face-defaults.md` passes the existing
`metal-window` gate with the display configured at its actual 120 Hz rate.
The fixture first presses the realm's ordinary Enter key to finish CHARACTER
selection, then allows ten seconds for its existing Coax quality governor.
An earlier picker-only run reached 117.4 fps but correctly failed movement;
it remains in `window-steady-120/` and is not gameplay evidence.

The actual gameplay run records 669 completed samples over 6.158239 seconds,
108.4726 fps, passing the unchanged minimum of 90% of the configured rate.
P95 frame interval is 16.670 ms. This is a loaded-host observation with thermal
state 1, low-power false, not a guarantee of one new frame every display tick.
The Coax governor selected a 640×360 drawing surface for the 640×360-point,
2× backing-scale window; native code does not choose a separate quality path.

Twenty actual key transitions reach frame submissions, the player moves,
and P95 key dispatch to GPU completion is 18.680 ms. This software endpoint
does not measure optical display latency. While status covers the world,
frames and completions remain 1,641, pending drains to zero, and received
room messages advance 567→597. After revealing and resizing to 900×600,
the world remains net OPEN with seat 0. The GPU ledger closes live/words 0/0.

The fixture also obtains the actual UNORM presented image and a JPEG through
the normal selector's retained display target. Complete rows are archived as
`window-play-120.json.gz`; the uncompressed receipt SHA-256 is
`ab4b78181821587d15bbe44c80dd5e1287c41136e70f922db9e7d98e082f362e`.
The original result, plan, FP16 graph, actual display, PNGs and JPEG remain in
`~/.openagents/aethyrforge/metal-host-12868/graphics-admitted-3f8/window-play-120/`.
The adjacent `window-play-120.txt` is the unmodified gate result.

This uses the exact source-joined 17a CPU units, corrected matching face,
admitted graphics products and ordinary shared precision selector. It does
not establish current-main CPU packaging or complete HTTP/session parity.
Both issues remain open and WebKit remains the desktop default.
