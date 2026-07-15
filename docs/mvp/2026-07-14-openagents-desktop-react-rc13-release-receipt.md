# OpenAgents Desktop React RC13 release receipt

Date: 2026-07-14

Tracking issue: [#8823](https://github.com/OpenAgentsInc/openagents/issues/8823)

Source commit: `a66b8d4ea7b1df3f6f753bf3786f466b43add902`

Candidate: `0.1.0-rc.13` for macOS ARM64

Result: the exact React-default candidate passed signing, notarization,
Gatekeeper, pristine-profile packaged smoke, and reversible
RC12-to-RC13 update/rollback/reinstall acceptance. No artifact or feed was
published by this ceremony.

## Exact candidate

| Artifact | Bytes | SHA-256 | Apple submission |
| --- | ---: | --- | --- |
| `OpenAgents-0.1.0-rc.13-arm64.dmg` | `304184884` | `e4f323601d44f24f68dbd36dcf2b0e8ff6558bc510ababa28f482ad01087ca63` | `82bcce59-02e0-4c5f-bd1e-651c111942ac` (`Accepted`) |

The app identity is `com.openagents.desktop`, version `0.1.0-rc.13`, ARM64,
team `HQWSG26L43`, and Developer ID authority `OpenAgents, Inc.`. The hardened
runtime is enabled. `syspolicy_check distribution` passed, and `hdiutil
verify` reported the post-staple DMG checksum valid.

The clean-source release preflight ran at exact `origin/main` and passed all
source, identity, attribution, artifact-set, renderer-remnant, absolute-path,
and credential gates. The artifact-bound rerun then passed:

- no unsigned-development marker;
- DMG accepted by Gatekeeper as `Notarized Developer ID`;
- stapled DMG ticket validation;
- deep/strict app signature validation;
- app accepted by Gatekeeper as `Notarized Developer ID`; and
- stapled app ticket validation.

## Packaged React proof

The accepted DMG was mounted read-only and launched with a fresh temporary
user-data root and the React smoke oracle. It proved:

- one React workbench surface and zero compatibility roots;
- real Chromium keydown/input delivery into the composer;
- a new local chat with typed model, reasoning, tool-start, tool-result, and
  assistant timeline items;
- bounded read-only Git review with no absolute-path or write-action leak;
- Runtime Gateway protocol v11;
- renderer reload restoration of the same durable six-item timeline; and
- lifecycle teardown with zero active owners.

The terminal result was `[openagents-desktop smoke] REACT OK` followed by
`lifecycle-teardown {"ok":true,"active":0}`. Provider processes were explicit
smoke fixtures; this is not a real-account Codex receipt.

## Reversible release lifecycle

The retained exact RC12 artifact had SHA-256
`edaaa642f42286005f60d3c5bd225d62c7a3551784d49d23f140bae6c63e564a`,
validated its stapled ticket, and was accepted by Gatekeeper as `Notarized
Developer ID`.

The production update client and real macOS update applier then passed the
public-safe eleven-step sequence:

1. install exact stapled RC12 into the uniquely named proof location;
2. generate and self-verify an RC13 manifest through the production-pinned
   release-signing seam;
3. stage digest-verified RC13 and recover it after update-host destruction;
4. atomically replace RC12 with exact notarized RC13;
5. refuse RC12 as a non-monotonic downgrade outside the rollback slot;
6. consume the retained slot and restore exact notarized RC12;
7. export a schema-valid redacted owner-only diagnostic receipt;
8. uninstall the reversible proof app;
9. reinstall exact stapled RC13 from the accepted DMG;
10. remove the proof app, rollback slot, staged bytes, diagnostics, mounts,
    and private state; and
11. report the exact RC12-to-RC13 sequence passed without deployment.

The driver exited zero. It never mutated `/Applications/OpenAgents.app` and
left no proof installation behind.

## Honest boundary

This receipt satisfies the signed/notarized artifact and
update/rollback/reinstall/cleanup receipt classes for #8823. It does not supply
the remaining real ordinary-session Codex journey, authoritative decision
journey, owner ProductSpec admission, independent visual/interaction review,
screen-reader and admitted-device accessibility review, or admitted-device
performance percentiles. Those remain open and are not waived.
