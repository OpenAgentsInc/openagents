# T3UI-10 remote/mobile/connect receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Baseline: `OpenAgentsInc/openagents@5d2e1215b0`
- Scope: remote environment, pairing, and mobile-client control

## Implemented

- Settings now includes a Connections route with remote-control status,
  environment connection, portable-session disclosure, short-lived mobile
  pairing, paired-client refresh, and exact client revocation.
- Every enabled action lowers through a typed shell intent, the schema-decoded
  preload bridge, trusted main sender validation, and the existing Codex
  experimental runtime authority/receipt model.
- Remote client list/revoke now accepts a public environment ref. Main resolves
  the raw environment id held in runtime memory; the renderer never receives
  or sends that private id after connection.
- The renderer projection drops pairing codes, installation identity, raw
  client ids, exec-server URLs, and private runtime fields. Pairing and client
  rows use only stable public refs and bounded presentation metadata.

## Proof

- Runtime tests prove correlated pairing, public-ref client listing, exact
  revocation, and reset-safe receipt behavior. Projection tests prove private
  fields do not survive decoding.
- Mounted tests cover route discovery, portable/SSH boundary copy, typed pairing
  status, and exact client-revocation intents.
- The `remote-connect` frame was manually inspected; all 22 committed frames
  pass with zero pixel drift.
- Desktop TypeScript, 213 serial test files (2,055 passing, 39 skipped), the
  production build, compatibility Electron smoke, React Electron smoke, and the
  visual gate pass.

## Boundaries

This packet does not copy T3's relay, account, cloud, or credential authority.
It stores no SSH password or private key in renderer state and labels the native
credential-broker gap. Live cross-machine owner evidence, responsive and
accessibility closure, installed signed evidence, and the final component
census remain later packets; this is not a T3 parity claim.
