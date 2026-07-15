# OpenAgents Desktop React RC16 release receipt

Date: 2026-07-14

Tracking issues: [#8817](https://github.com/OpenAgentsInc/openagents/issues/8817)
and [#8823](https://github.com/OpenAgentsInc/openagents/issues/8823)

Application source commit: `c12a0d4422892d5ce9198f1447be6d2f0c85c176`

Acceptance contract: ProductSpec revision 3, SHA-256
`4ce0e03b8cb33a61abf2caf7e059a0ddbcc4a1265ca7068fbf2d0e40319878e9`,
with the owner-directed assurance gates in
`openagents-desktop-mvp-phase-2-react-codex-workbench.assurance-gates.md`.

Candidate: `0.1.0-rc.16` for macOS ARM64

Result: passed. No artifact, tag, GitHub release, update feed, or public claim
was published.

## Exact candidate

| Artifact | Bytes | SHA-256 | Apple submission |
| --- | ---: | --- | --- |
| `OpenAgents-0.1.0-rc.16-arm64.dmg` | `304204258` | `9b22c99b076587920e0c9dc92705dd2bec0c4d5b6cdb8c8f89f464cacbfc5a71` | `555ebf49-e1f5-4d51-9c3e-34455bd94630` (`Accepted`) |

The nested app and outer DMG are stapled. The app identity is
`com.openagents.desktop`, version `0.1.0-rc.16`, ARM64, team `HQWSG26L43`, and
Developer ID authority `OpenAgents, Inc.`. Hardened runtime is enabled.
`codesign --deep --strict`, app and DMG Gatekeeper assessment, stapler
validation, and `hdiutil verify` all passed.

The release preflight passed clean-origin-main, strict RC15-to-RC16 version
monotonicity, identity, attribution, artifact-set, forbidden-remnant,
source-path, signing-credential, unsigned-marker, Gatekeeper, signature, and
ticket gates.

## Owner-directed conversation gates

The exact source and installed React smoke prove the revision-3 hierarchy:

- authored user and assistant messages are the primary transcript;
- usage, session, context, connection, metadata, and successful lifecycle
  scaffolding are not conversation messages;
- reasoning, tools, approvals, and collaboration use compact work rows with
  raw details collapsed by default;
- settled consecutive work folds behind `Worked · N activities`, while active
  work and one bounded working indicator remain visible;
- same-key streams replace in place, preserve manual reader position, and
  follow only at the live edge;
- redacted reasoning placeholders remain absent while actual authored loss,
  gaps, interruption, and failures remain truthful;
- the shadcn session `ScrollArea` owns its bounded flex/viewport scroll; and
- the approved Khala semantic colors remain canonical, with reduced-motion
  handling for the working indicator.

The focused hierarchy suite passed 19 tests. The complete Desktop gate passed
141 files, 1,355 tests, the production build, Electron smoke, reload recovery,
and zero-owner teardown.

## Mounted installed journey

The stapled DMG was mounted read-only and its packaged app ran the React-default
smoke against a fresh temporary profile. It proved exactly one React workbench
and zero compatibility roots, real Chromium key/input delivery, a streamed
turn, a provider-originated approval correlated as request `91`, completion
withheld until the accepted response, bounded read-only diff review with no
absolute-path leak, renderer reload restoration, and lifecycle teardown with
zero active owners.

The deterministic protocol-speaking app-server peer is the installed release
oracle. Ordinary logged-in-session custody, inherited `CODEX_HOME` clearing,
and native runtime mapping remain separately covered by the current Desktop
contract suite; no external Codex interface was counted as success.

## Assurance and performance

The admitted Phase 1 assurance run was refreshed against current main:

- assurance digest:
  `sha256:66e1b49d3089b141a9bd5fb6221d002a0d364259ab719a46a254a507fb0dee72`;
- manifest digest:
  `sha256:afd25a5d9f9a8442773d3d18dbda1b4feae4a29e4181a9afc8f1d9cc72cbdb17`;
- 18 of 18 candidates confirmed;
- 18 of 18 falsifiers refuted; and
- evidence-index digest:
  `sha256:bc12b8280dce9992a7d4a54d795610b8ce5f76dedb96a4e8d36167afe598c124`.

The seven-run Node 24 macOS ARM64 startup receipt passed its budgets:
first paint median `381.87 ms` / p95 `384.95 ms`, shell mounted median
`417.77 ms` / p95 `420.86 ms`, and capability ready median `422.77 ms` / p95
`424.86 ms`. The release gate allows `1500 ms` median and `2500 ms` p95 warm
interactive startup.

Keyboard behavior, first input, disclosure semantics, focus stability,
screen-reader naming, reduced motion, token-only color authority, responsive
minimum-window behavior, bounded Markdown, long-timeline update, and teardown
remain executable Desktop gates rather than screenshot-only claims.

## Reversible lifecycle

The production updater passed the exact notarized RC15-to-RC16 sequence:

1. install exact stapled RC15 into the reversible proof location;
2. create and self-verify the production-pinned signed RC16 feed;
3. recover digest-verified staged RC16 after update-host destruction;
4. atomically replace RC15 with exact notarized RC16;
5. refuse RC15 as a non-monotonic downgrade outside the rollback slot;
6. consume the retained slot and restore exact notarized RC15;
7. export a schema-valid public-safe owner-only diagnostic receipt;
8. uninstall the proof app;
9. reinstall exact stapled RC16 from the accepted DMG; and
10. remove the app, rollback slot, staged bytes, diagnostics, mounts, and
    private state.

The first reinstall attempt exposed a real driver defect: detach-by-mount-path
could leave the whole disk device attached and block reattachment. The driver
now captures and detaches the `/dev/diskN` device, with forced-device detach as
the fallback. The complete lifecycle was rerun from step one and passed.

## AC-1 through AC-14 disposition

| Criteria | Disposition | Receipt |
| --- | --- | --- |
| AC-1–AC-2 | Pass | one React root, declared boundary, one Effect snapshot subscription, exact-once intent/lifecycle tests |
| AC-3–AC-4 | Pass | ordinary local Codex start plus metadata-first session management, search, selection, paging, and restart recovery |
| AC-5 | Pass | owner-directed conversation hierarchy, stable causal keys, folded work, truthful gaps/terminal state, anchored prepend |
| AC-6–AC-7 | Pass | first input/IME/command identities and installed authoritative decision ordering |
| AC-8 | Pass | bounded exact read-only review and refusal/privacy tests |
| AC-9–AC-10 | Pass | keyboard/focus/naming/contrast/reduced-motion/minimum-window and truthful recovery-state gates |
| AC-11 | Pass | import/schema/private-authority and artifact scans |
| AC-12 | Pass | installed stream/decision/review/switch/reload/resume fixture journey plus ordinary-session custody contracts |
| AC-13 | Pass | React workbench exclusive; compatibility backend absent from the installed scoped journey |
| AC-14 | Pass | signed/notarized artifact, performance, Gatekeeper, update/rollback/reinstall, cleanup, and zero-owner teardown |

## Boundary

RC16 is a release-ready candidate for the admitted revision-3 Desktop
transition. This receipt closes the implementation and candidate-proof issues.
It does not itself publish RC16, change the live update feed, create a GitHub
release, or authorize broader web/mobile/Fleet/editor/terminal claims.
