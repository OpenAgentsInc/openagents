# Shared desktop chat cards

Issue #10009 mounts router offers, suggested questions, working and retry
states, and the phone's shared Gym card values in the desktop transcript.
The cards scroll with messages, keeping the native composer on screen. The
presentation reimplements Zeron's design in Rust Native; no Zeron code or agent
harness is copied.

The shared app creates the chip labels and typed actions. A completed reply's
computer-lane judgment can offer Coder even without router metadata. A streaming,
failed, or stopped reply does not offer follow-ups or a Coder action. Suggested
questions disappear by their stable answer ID or normalized words. The host
persists used IDs in the encrypted cache and retries interrupted writes.

The desktop and phone use the same `CardView` values for capability, draft,
result, news, check, credit, and missing-capability cards. Native transcript
buttons carry only semantic node keys. A matching pointer release resolves
against the current action registry and conversation revision; replacing a row
cancels a press. Disabled
buttons and unknown IDs cannot activate. Ordinary answer text carries no router
source tags or prepared-answer metadata. Card provenance remains an explicit
part of the shared card value.

A suggestion or follow-up sends its exact message through the existing hosted
chat control path. Its response leaves an unrelated native draft intact.
**Change it** prefills the composer, and a card's suggested question can start
its requested fresh chat. Offered navigation uses the adapter's destination:
Computers, Settings, or the Grid. Wallet and phone-specific destinations say to
open them on the phone. Coder execution, connected-computer commands, and Gym
runner actions are mounted here as typed offers; their execution integrations
belong to #10014, #10015, and #10020. They grant no execution authority here.

The hosted request now identifies the desktop surface and client. The worker
already accepts that context; the local parser retains its read-only command
allowlist. The window owns no signing credentials. Shared card copy uses
“this device” so a desktop draft is not described as living on a phone.

## Checks

On pinned Rust 1.97.1:

- Shared core: 28 tests pass; one public smoke remains opt-in. The new checks
  cover used-ID recovery and desktop context without computer authority.
- Shared app: 68 tests pass. Each of the seven recorded worker card fixtures
  produces the same values through the phone controller and desktop card state;
  instance-specific button IDs are excluded from that value comparison.
- Desktop: 108 tests pass; four opt-in timings and live smokes remain ignored.
  A pointer click sends **Who are you?** through the fixture control host and
  persists its used ID. Every worker card mounts and paints.
- Native desktop adapter: 43 tests pass, including enabled buttons, matching
  release, canceled presses after replacement, painting, and existing editing.
- Shared layout: 22 tests pass; one opt-in timing case remains ignored.
- Phone library: 142 tests pass; 19 opt-in cases remain ignored. The two Coder
  judgment fixtures verify that the extracted policy preserves phone behavior.
- Private host control: eight integration tests pass on temporary roots.
- Targeted formatting, Clippy with warnings denied, and locked phone compilation
  pass. Tests use no owner host, home, keychain, or persistent chat lists.

The offline 3,300-row native fixture ran at 1,200 × 840 points, 2× scale, with
Verse visible. Across 115 samples per phase, scroll p99 is 4.359 ms, streaming
p99 is 7.039 ms, and sidebar p99 is 4.224 ms. All are below the 8.3 ms target.
Idle CPU is 3.18%; peak resident memory is 185.5 MiB. Application work and CPU
frame submission are included; GPU completion and scanout are not. The
[numeric report](native.json) retains every sample. Removing extra snapshot
clones reduced the streaming cost measured in #10008.

## Fixture captures

These captures use recorded data and an in-process fixture host:

- [Capability](tool.png)
- [Draft](draft.png)
- [Result](result.png)
- [News](news.png)
- [Check](check.png)
- [Credit](credit.png)
- [Missing capability](capability.png)
