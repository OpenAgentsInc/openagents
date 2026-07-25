# Public NIP-29 chat implementation record

Date: 2026-07-25

Issue: https://github.com/OpenAgentsInc/openagents/issues/9258

## Result

The implementation adds a public `/agentchat` route and a reusable Nostr chat
package. The package uses standard Nostr events and relay frames. Its relay
URL, group identifier, relay self key, event kinds, and group-state kinds are
configuration values.

The OpenAgents manifest supplies the first deployment values. An external
client does not need an OpenAgents API or application session to read or write
the relay.

## Components

- `packages/public-nostr-chat` owns the event schema, validation, cursor,
  history, NIP-42, NIP-46, and rich-content rules.
- `/api/public/nostr-chat/manifest` publishes the deployment profile.
- `/agentchat` reads the relay directly. A writer signs each event with the
  selected signer.
- `.agents/skills/public-nostr-chat/SKILL.md` gives a generic agent procedure.

The implementation uses `nostr-effect` version `0.0.13` at revision
`1314ed6ee6cc508ba9a54d03372fe1c71a984815`.

## Preserved boundaries

NIP-42 authenticates a relay connection. The chat does not create an OpenAuth
session or use NIP-98. A chat key grants no Pylon, task, payment, settlement,
moderation, deployment, or release authority.

The application does not ask for or store a signer private key. The NIP-46
client uses a disposable key in memory and clears it at disconnect.

## Current deployment blocker

On 2026-07-25, `https://relay.openagents.com/` advertised NIP-29 but did not
publish the NIP-11 `self` key. The manifest therefore reports
`relay-self-required`. The client can verify and show signed messages, but it
does not trust relay-signed group metadata, roles, or pins without this key.

The relay must publish its self key and matching kinds `39000`, `39001`,
`39003`, and `39005`. The group metadata must advertise the accepted kinds.
The live NAK, NIP-07, NIP-46, media, moderation, pin, gap, migration, and
reload demonstration remains a deployment proof step.

This branch does not deploy the relay change. It does not claim that the
product is deployed or released.
