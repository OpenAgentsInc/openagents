# T3UI-08 browser Preview receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Baseline: `OpenAgentsInc/openagents@9309e0c407`
- Scope: safe local/file Preview workbench and typed annotation

## Implemented

- Preview is a persisted transcript-preserving workbench tab and appears only
  when a generation-owned terminal has announced a local server.
- The T3-shaped chrome mounts disabled back/forward controls, refresh discovery,
  exact target selection, a read-only verified address, confirmed system-browser
  open, annotation, capture/recording disposition, and responsive/mobile/tablet/
  desktop viewport framing.
- The stage supports a ready/waiting/empty local-server journey and previews the
  exact active editor file without granting new filesystem access.
- Annotation validates the exact terminal session and ready port, is bounded to
  2,000 characters, enters removable one-turn composer state, and is delivered
  as explicitly untrusted preview metadata.

## Proof

- Mounted shell tests cover capability-backed Preview admission, ready target,
  browser chrome, device selection, annotation entry, and preserved transcript.
- Shell intent tests prove a valid exact ready target attaches and lowers through
  the untrusted provider envelope; unknown or unready targets fail closed.
- The visual lane admits `browser-preview`; all 20 canonical frames were
  regenerated and the new frame was inspected.
- Desktop TypeScript passes. The full serial suite passes 211 files and 2,042
  tests with 39 skipped after two unrelated timer-budget flakes passed on rerun.

## Boundaries

The existing enforced Desktop contract allows only terminal-announced localhost
targets and opens them out of process after confirmation. This packet does not
weaken that invariant: arbitrary address navigation, page DOM, cookies,
credentials, capture, recording, and browser automation remain unavailable and
are labeled as such. Enabling them requires a separately admitted isolated
browser host with its own sandbox, lifecycle, evidence store, and policy tests.
Settings convergence, remote/mobile, installed signed evidence, and T3 parity
remain later packets.
