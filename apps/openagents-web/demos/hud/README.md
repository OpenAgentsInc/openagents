# Aethyr HUD laboratory

A reusable Coax HUD library with a component catalog and Grid command,
flight navigation, engineering and communications compositions. Its synthetic
fixtures are independent of the homepage's verified live observations.

Run from the repository root:

```sh
coax/web/tool.sh apps/openagents-web/demos/hud/build.coax
coax/web/serve.sh apps/openagents-web/build/hud --listen 127.0.0.1:8815 --build 'coax/web/tool.sh apps/openagents-web/demos/hud/build.coax'
```

Review at <http://127.0.0.1:8815/>. The second command keeps the dev server
in the foreground. Alternatively, add `--serve` to the first command to start
the same server detached; the tool prints its PID. Stop that process when the
review is finished.

The tool generates an ignored standalone app under
`apps/openagents-web/build/hud`. It compiles the demonstration with the existing
unmodified `projects-face.c`, generates the projects shader from its existing
Coax source, and writes the minimal Coaxial canvas shell and accessible text
twin. All new application and build logic is Coax. No film images or footage
are bundled. The shell reuses the standard browser host's platform behavior.

Generated source symlinks connect the standard dev server's file watcher to
the demo, build tool, shared library, frame geometry and font. Saving those
files rebuilds the page. If a new source file is added, connect it in the build
tool and restart the server so the watcher discovers it.

| Key | Action |
| --- | --- |
| `1` | Component catalog |
| `2` | Grid command |
| `3` | Flight navigation |
| `4` | Engineering |
| `5` | Communications |
| `M` | Monochrome or cyan accents |
| `Space` | Pause or resume the demonstration clock |
| `Tab`, `Shift Tab` | Move focus |
| `Enter` | Activate the focused control |
| `Escape` | Close the enlarged component |
| `R` | Reset local demo readings and selections |
| Wheel, `Page Up`, `Page Down` | Scroll the component catalog |

Click any fully visible component card to inspect it. The Grid's contact
selection updates its drawing and register together. Flight supports contact
selection, range modes and an armed route preview. Engineering links the
selected subsystem to its matrix. Communications command tiles update the
local transcript and message state; no external commands are sent.

Run the focused checks against the running server:

```sh
coax/bin/coax test apps/openagents-web/demos/hud/projects.coax --library
coax/bin/coax test apps/openagents-web/demos/hud/build.coax --library
coax/web/tool.sh apps/openagents-web/demos/hud/check.coax
```

The host supplies reduced motion. The generated hidden text describes controls
and scope; it does not claim the complete dynamic accessibility tree planned
in Aethyr chapter 15.

Visual research starts with [The Expanse UI Design — HUDS+GUIS](https://www.hudsandguis.com/home/2021/theexpanse).
The library extends the existing Grid frame, monochrome strokes and restrained
cyan with original navigation, instrumentation and communications primitives.

Headless verification uses the repository's muted Chrome path:

```sh
coax/web/tool.sh shot http://127.0.0.1:8815/ /tmp/aethyr-hud.png 1600x1000 2000
```

Do not open a visible browser window. The page remains served for an owner's
manual review.

The Coax verification tool checks scene frames at five phone, landscape and
desktop viewports, vertex budgets, finite output, action geometry, scrolling,
modal focus, cancelled presses and all six communications commands. It also
drives the actual page Wasm in muted headless Chrome using real keyboard and
pointer events, including pause, reduced motion and the phone layout:

```sh
coax/web/tool.sh apps/openagents-web/demos/hud/check.coax
```

Use `--native` or `--browser` to run one side independently, or `--regressions`
for just the native interaction checks. The browser observer is confined to
the test tool and reads the current state's documented scalar prefix; update
it if that state layout changes.
