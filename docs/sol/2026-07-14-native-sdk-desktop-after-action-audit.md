# Native SDK desktop experiment: after-action audit

- Class: historical analysis and decision record
- Status: closed — experiment removed; Electron remains the sole OpenAgents Desktop host
- Snapshot: 2026-07-14 at OpenAgents `f9ff746ba`, before retirement cleanup
- Owner: OpenAgents Desktop architecture
- Dispatch: no
- Revival: explicit owner decision only

## Decision

OpenAgents will not proceed with Vercel's Native SDK for the Desktop product at
this time. The shipping architecture remains Effect Native and React inside
Electron, with Electron owning the application window, privileged host
services, packaging, signing, updates, and lifecycle.

The bounded Native SDK experiment and all owned integration code have been
removed. This record replaces the earlier adoption audit. It is deliberately an
after-action document, not a proposal, roadmap, or dormant implementation queue.

The external reference checkout at `projects/repos/native` remains read-only
research material. It is not an OpenAgents dependency or an approved runtime
target.

## What we attempted

The experiment progressed beyond a static architectural review:

1. We initialized a real Native SDK application and pinned the upstream runtime
   rather than mocking its API.
2. We built a macOS ARM64 Zig host with a retained Metal-rendered native rail,
   native windowing, drag-and-drop input, and Native SDK automation.
3. We embedded the real Effect Native Desktop center pane in a WebView and
   reused the Desktop view state, typed intents, handlers, command registry, and
   product styling.
4. We added a Node sidecar so the alternate host could reach selected real
   Desktop services and repository-backed workspace authority.
5. We added a headed host smoke, exact Node/Zig checks, artifact digests, and a
   target-specific AssuranceSpec adapter.
6. We moved the hybrid toward the real application by wiring persistent
   repository identity and selected Desktop-service bootstrap behavior.

The resulting topology was:

```text
Native SDK / Zig window
├── retained native canvas rail
├── WKWebView
│   └── Effect Native → React DOM Desktop pane
└── Node sidecar
    └── selected OpenAgents Desktop services and repository authority
```

This was a useful proof. It was not a simpler replacement for Electron.

## What worked

The experiment produced several concrete positive results:

- Native SDK created and drove a real macOS window with a retained GPU canvas.
- Its native automation could inspect the retained component tree, drive input,
  exercise file drop, and capture deterministic Native-owned surfaces.
- A Native-owned rail and the real Effect Native product pane could coexist in
  one headed application.
- The portable Effect Native view contract was strong enough to reuse real
  application state and typed intent handling across the WebView boundary.
- A closed sidecar protocol could bootstrap selected Desktop services without
  giving renderer code arbitrary process access.
- The exact headed verification passed once: 14 frontend tests, 7 sidecar
  tests, Zig tests/build, and the Native host smoke.
- Durable repository identity could survive the alternate-host boundary.

These are valid engineering findings. They do not establish product parity.

## What failed the adoption bar

### 1. The alternate host multiplied runtimes

Electron already supplies the JavaScript runtime, browser renderer, window
host, IPC boundary, packaging lifecycle, and mature platform integrations used
by the product. The Native path needed three cooperating domains—Zig/Native
SDK, WKWebView, and Node—to preserve the real application.

That added a second host protocol and a second renderer bridge while retaining
both a browser engine and Node. The experiment therefore did not realize the
main expected benefit of a native rewrite: fewer runtimes and less host
complexity.

### 2. Parity meant recreating the Electron main process

The Desktop main process owns security-sensitive and lifecycle-sensitive work:
workspace admission, Codex process custody, terminal and Git operations,
credentials, dialogs, recovery, updates, release policy, and diagnostics.
Moving the UI did not move those responsibilities.

The sidecar was necessarily becoming a reimplementation of the Electron
service host. Every additional parity feature expanded protocol versioning,
shutdown ordering, crash recovery, authority checks, diagnostics, and test
surface. That is duplicated architecture, not host consolidation.

### 3. The strongest automation stopped at the WebView boundary

Native SDK automation was compelling for its retained canvas. The actual
OpenAgents product pane, however, remained WebView content. It did not acquire
the same semantic inspection and pixel-proof surface automatically. The test
harness needed composited screen capture and special bridge logic to reason
about both halves.

The last visual iteration also exposed layout friction in Settings when the
real pane was composed inside the hybrid viewport. That issue was fixable, but
it was evidence that each host introduces another layout and lifecycle context
that must be owned and tested.

### 4. Assurance remained far from product parity

At the last pushed experiment commit, only **1 of 18** frozen MVP criteria had
genuine target-specific confirmation: CW-AC-03. The other 17 criteria remained
gaps. Work toward a second diagnostics observation was uncommitted when the
experiment was stopped and is not counted as evidence.

The experiment did not prove, in the Native target:

- real Codex session custody and complete turn streaming;
- terminal, Git, child-agent, approval, steer, queue, and recovery behavior;
- ProductSpec authoring and dispatch;
- the complete workspace/document lifecycle;
- signed and notarized distribution, update application, and rollback;
- release-grade crash, restart, and data migration behavior; or
- all 18 AssuranceSpec criteria against one installable artifact.

A passing bounded host smoke cannot substitute for those product obligations.

### 5. Product and platform timing were unfavorable

The audited Native SDK revision was `f7aa92af6dcece250feba852af4d22e7f5429312`
(`v0.5.1`) with Zig `0.16.0`; the Desktop JavaScript runtime was Node
`24.13.1`. The SDK was moving quickly, desktop was its strongest surface, and
the mobile story was still experimental. Exact pins reduced surprise but also
made upstream API churn and local maintenance an OpenAgents responsibility.

That trade was not justified while the existing Electron release path already
supports the product.

## Why Electron wins now

Electron is not being retained merely because it is familiar. It currently has
the lower total architectural cost for this application:

| Concern | Electron today | Native hybrid experiment |
| --- | --- | --- |
| Effect Native / React UI | direct, shipping path | still required inside WKWebView |
| Node services | in the established main-process topology | separate sidecar and protocol |
| Process count | Electron's known model | Native host + WebView + Node |
| Desktop parity | existing product implementation | 1/18 criteria confirmed |
| Packaging and updates | existing signed-release design | not demonstrated |
| Security ownership | existing typed preload/main boundary | new bridge plus sidecar boundary |
| Automation | mature product tests and Electron smoke | excellent canvas automation, incomplete WebView coverage |
| Maintenance | one shipping host | two host implementations plus upstream pin churn |

The right decision is to improve the existing Electron host and Effect Native
renderer rather than fund two incomplete Desktop architectures.

## Lessons retained without retaining code

The experiment identified useful design ideas, but they remain references—not
dependencies or dormant implementation seams.

- **Closed typed bridges:** keep renderer-to-host protocols small, versioned,
  serializable, and fail-closed.
- **Host-owned authority:** UI components request capabilities; they do not own
  filesystem, process, credential, or release authority.
- **Durable opaque identity:** repository, session, and workspace references
  should survive reload and host boundaries without exposing private paths.
- **Privacy-safe diagnostics:** project bounded status rather than raw logs,
  environment values, or process output.
- **Deterministic native fixtures:** Native SDK's retained component fixtures,
  stable identity rules, automation commands, and snapshot discipline are good
  inspiration for Effect Native conformance tests.
- **Opinionated components:** scroll behavior, menus, controls, tables, split
  panes, overlays, focus rules, and keyboard contracts can inform Effect Native
  component specifications. Adopt the behavior contract and conformance tests,
  not Native SDK props or a runtime dependency.
- **Canvas renderer research:** a future Effect Native canvas renderer should
  remain independently swappable and prove real product value before it is
  attached to Desktop.

In short, the component-level opportunity is to study and re-express good
behavior behind Effect Native's owned schemas and tokens. It is not to leave a
Native SDK adapter hidden in the tree.

## Removed surfaces

This retirement removes:

- `apps/native-sdk-effect-native-spike/`;
- the Desktop Native sidecar contract, implementation, and tests;
- the portable WebView-only Desktop renderer entry and split center-pane seam;
- the Native SDK AssuranceSpec adapter, criterion catalog, target selector, and
  target tests;
- Native workspace and lockfile membership;
- generated Native build, host-smoke, and assurance artifacts; and
- the superseded adoption audit.

Electron's existing application contracts, renderer, services, and general
AssuranceSpec implementation remain.

## Revival boundary

Do not reopen Native SDK work through opportunistic component imports, hidden
host flags, new sidecar methods, or a background parity queue. Revival requires
an explicit owner decision and a new dated analysis.

At minimum, a future proposal must show:

1. a named product goal that Electron cannot meet economically;
2. fewer privileged processes or materially simpler authority—not merely a
   different window host;
3. a credible Effect Native renderer path that does not depend on embedding the
   current product wholesale in a WebView;
4. complete target-specific proof for all 18 frozen MVP criteria;
5. signed, notarized, update, rollback, crash, and migration evidence;
6. a supported platform matrix aligned with Desktop and mobile strategy; and
7. a maintenance budget for exact upstream pins and API churn.

Until those conditions change, Electron is the sole Desktop host and Native SDK
is research material only.

## Verification after retirement

The retirement was verified against the remaining owned surfaces:

- the complete OpenAgents Desktop verify command passed: 138 test files, 1,342
  tests passed, 39 retired tests skipped, production build passed, and the
  built Electron smoke completed through lifecycle teardown;
- the scoped AssuranceSpec suite passed: 17 test files and 190 tests;
- the Sol manifest and full Sol documentation policy/link check passed with
  112 governed documents and zero product-document issues;
- the public AssuranceSpec package was repacked and its distribution receipt
  rebound after removing the Native adapter;
- the workspace lockfile was regenerated without the spike importer; and
- an executable-source search found no Native spike, sidecar, portable-host,
  target-adapter, or superseded-audit references outside this historical
  record.

The first Desktop verification attempt exposed an existing fixed-20-ms
Codex-connect timing flake. Its isolated rerun passed all 27 tests, and the
subsequent complete Desktop verify passed. No production code was weakened to
silence it.

## Final disposition

The experiment answered the central question. Native SDK has strong retained
UI and automation ideas, and Effect Native can learn from its component
discipline. For the real OpenAgents Desktop, the tested hybrid increased
runtime and authority complexity before reaching parity. Continuing would have
created a second product host without displacing Electron.

Decision: remove the experiment, retain the lessons in this record, and invest
in the Electron + Effect Native path.
