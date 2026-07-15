# OpenAgents Desktop React RC15 release receipt

Date: 2026-07-14

Tracking issue: [#8823](https://github.com/OpenAgentsInc/openagents/issues/8823)

Source commit: `9bb0bbb94909f5b0b3e371972335a7c4df850a44`

Acceptance contract: ProductSpec revision 1 at
`de1180b2da937922c2a8724915cf761f8fb78617`, SHA-256
`b88456951753e5a69b9a2390ad18d0fdecd1e1fbfcf65f2f6ddd7a5f1f060d41`

Candidate: `0.1.0-rc.15` for macOS ARM64

Result: the exact React-default candidate passed signing, Apple notarization,
Gatekeeper, mounted-DMG React smoke, the complete Desktop regression gate, the
18-obligation assurance run, and reversible RC13-to-RC15
update/rollback/reinstall acceptance. No artifact, tag, release, or feed was
published by this ceremony.

## Exact candidate

| Artifact | Bytes | SHA-256 | Apple submission |
| --- | ---: | --- | --- |
| `OpenAgents-0.1.0-rc.15-arm64.dmg` | `304197486` | `f604ac46dc9ca231b9e2840e89a13c857050c59f434465a326c5739b0837c784` | `0224d9f5-5a65-4790-926c-0e73e5eb2c1d` (`Accepted`) |

The app identity is `com.openagents.desktop`, version `0.1.0-rc.15`, ARM64,
team `HQWSG26L43`, and Developer ID authority `OpenAgents, Inc.`. The hardened
runtime is enabled. The post-staple DMG passed `hdiutil verify`; both app and
DMG passed Gatekeeper and stapler validation; the app passed deep/strict
signature validation.

The artifact-bound preflight ran from clean exact `origin/main` source commit
`9bb0bbb949` and passed all source, monotonic-version, identity, attribution,
artifact-set, forbidden-remnant, absolute-path, credential, signature,
Gatekeeper, and ticket checks.

## React development loop and production boundary

The source adds the reference-shaped desktop development loop: one strict
loopback Vite server at `127.0.0.1:5734`, React Fast Refresh/HMR, Electron
launch only after the server listens, custom-protocol asset proxying only in
an unpackaged process, and an isolated `OpenAgents Dev` profile. The packaged
candidate does not contain or consult the dev-server URL; it continues to
serve the signed static renderer through `openagents-app://renderer`.

The complete Node 24 Desktop gate passed 140 test files, 1,347 tests, the
production renderer build, and real Electron smoke/reload/teardown. The
source-bound assurance run then confirmed 18 of 18 candidate observations and
refuted 18 of 18 falsifiers. Its evidence-index digest is
`sha256:65a9e09a64aaa68e8937830e766da6e45154472060c653f5a8d7a44e52a325ce`.

## Mounted React and decision proof

The accepted DMG was mounted read-only and launched with a fresh temporary
user-data root and the React smoke oracle. It proved:

- exactly one React workbench surface and zero compatibility roots;
- real Chromium keydown/input delivery into the composer;
- typed model, reasoning, tool-start, tool-result, and assistant timeline
  items;
- a provider-originated command-approval request correlated as request `91`;
- an `accept` response returned before provider completion was emitted;
- bounded read-only Git review with no absolute-path or write-action leak;
- renderer reload restoration of the durable conversation and selection; and
- lifecycle teardown with zero active owners.

The terminal result was `[openagents-desktop smoke] REACT OK`, followed by
`lifecycle-teardown {"ok":true,"active":0}`. The provider peer was the
deterministic protocol-speaking smoke fixture, not a real-account Codex
session.

## Reversible release lifecycle

The currently published RC13 artifact was downloaded from the tracked release
manifest and matched SHA-256
`e118c704447228e3550de835a5105a9df85836e28f70872b6bec61d1a93f1556`
and byte length `303959067`. Gatekeeper accepted it as a notarized Developer
ID image and its staple validated.

The production update client and real macOS applier then passed:

1. install exact stapled RC13 into the uniquely named proof location;
2. generate and self-verify an RC15 manifest through the production-pinned
   release-signing seam;
3. recover digest-verified staged RC15 after update-host destruction;
4. atomically replace RC13 with exact notarized RC15;
5. refuse RC13 as a non-monotonic downgrade outside the rollback slot;
6. consume the retained slot and restore exact notarized RC13;
7. export a schema-valid, public-safe, owner-only diagnostic receipt;
8. uninstall the reversible proof app;
9. reinstall exact stapled RC15 from the accepted DMG; and
10. remove the proof app, rollback slot, staged bytes, diagnostics, mounts,
    and private state.

The driver exited zero and reported the exact RC13-to-RC15 sequence passed
without deployment. It never mutated `/Applications/OpenAgents.app`.

## Honest boundary

This receipt supplies the admitted ProductSpec binding, deterministic
provider-originated authoritative decision, signed/notarized artifact,
packaged React journey, complete current regression/assurance run, and
update/rollback/reinstall/cleanup evidence classes for #8823.

It does not supply a real ordinary logged-in Codex-session journey,
independent visual/interaction acceptance, admitted screen-reader/device
accessibility evidence, or admitted-device performance percentiles. It does
not authorize publication or public launch claims. Those remaining close-rule
classes stay open and are not waived.
