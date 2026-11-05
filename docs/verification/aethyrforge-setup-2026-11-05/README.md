# Aethyrforge covered-world pause and native tool panels (#12865)

Measured 2026-11-05 on the owner's MacBook Pro, Apple M5 Max, 18 CPU cores,
128 GB, macOS 26.4 (25E246). Source base: `53bfe38a88` plus this change.
The world/pack and native room are the installed Aethyrforge artifacts from
`c6caa2ec19`; this change replaces their platform host and tool UI. No world,
shader or room arithmetic was changed for this measurement.

The app and world view use **0.58% of one CPU core with setup open**, compared
with **19.53% in play** (97% lower). Setup/status/minimize/hide produce no world
frames, GPU submissions or pending GPU frames. The native tool panels produce
no idle draws or GPU execution. The separate native room continues delivering
about 30 frames per second. Both panels are Coax GPUI geometry/text, drawn with
the same native Zeron atlas, Metal pipeline and shaders as OpenAgents desktop.

| Phase | App CPU, % of one core | WK world CPU, % | World callbacks | GPU submissions | Room frames |
| --- | ---: | ---: | ---: | ---: | ---: |
| Play | 2.99 | 16.54 | 996 | 995 | 255 |
| Setup | 0.23 | 0.35 | 0 | 0 | 249 |
| Status | 0.22 | 0.30 | 0 | 0 | 249 |
| Status after reload | 0.24 | 0.33 | 0 | 0 | 247 |
| Minimized | 0.29 | 0.42 | 0 | 0 | 249 |
| Resumed | 3.06 | 18.14 | 991 | 914 | 252 |
| Hidden | 0.26 | 0.37 | 0 | 0 | 247 |
| Unhidden | 3.17 | 15.07 | 987 | 952 | 247 |

Each row measures about 8.3 seconds after a 1.2-second settling interval; raw
durations and observations are in [result.json](result.json). CPU uses
`proc_pid_rusage` user+system Mach ticks converted to nanoseconds. The independent
POSIX process CPU clock agrees at ratio 1.00000: [clock result](cpu-clock.log).
CPU columns cover the app and its WK content process, separately from the native
room that deliberately continues ticking. GPU observations are actual world
submissions/queue completions and the panels' Metal command-buffer GPU times.
The IOKit utilization samples are system-wide and are not used to attribute
GPU busy time to this app; their medians were zero in this run.

The actual visible first frame completed before setup paused it. Reload under
the status panel obtained another real first frame, then paused again. The test
also checked minimize, hide and both resumes. An initial reload experiment found
that loading the same fragment URL could become a same-document navigation;
Reload now calls WebKit's actual reload operation and reacquires readiness.

The native images were inspected: [setup](setup.png), [status](status.png).
Visible labels/backgrounds come from `chrome.coax`; AppKit's transparent buttons
provide input and accessibility. Both panels redraw on change, with no idle
animation loop. Their samples show zero extra panel draws and zero panel GPU
seconds. Realm audio suspends with its loop. Socket delivery remains independent.

## Repeat

Build the desktop app with `apps/aethyrforge-desktop/build.sh`, then run:

```sh
mkdir -p /tmp/aethyrforge-setup-profile
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge \
  --setup-profile /tmp/aethyrforge-setup-profile/result.json
```

This intentionally opens a visible native window. It uses real WebKit animation
callbacks, an isolated world, muted audio, no physical microphone, no model load
and no coding provider. It writes the report and panel images, stops its child
servers and exits. The harness is
`apps/aethyrforge-desktop/Sources/SetupProfile.swift`; the lifecycle admission is
`world-visibility.coax`, and the platform effect is `coax/web/host.js`.

Coax chrome checks and the session's generated-C build passed. The visibility
authority's 32-case fact matrix passes and all [four mutants](visibility-mutate.log)
are killed. OpenAgents desktop's complete Swift sources typecheck against the
extracted renderer/atlas, and the shared Metal shader compiles with Apple's SDK.
The repository tool audit initially reported 32 existing missing-reason headers
on the measurement base; after rebasing onto the independent tool ports it
reports 25, all elsewhere. Initial and saved-setup native checks pass
([first](setup-smoke.json), [reopen](setup-reopen.json)), as does the real
[Enter/Coder input check](input-smoke.json). After rebasing and packaging the
updated browser host, the [hotbar/reload check](hotbar-smoke.json) also passes,
including play-mode entry, closed editor and persisted key assignments.
