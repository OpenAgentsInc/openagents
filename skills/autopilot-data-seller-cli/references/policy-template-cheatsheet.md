# Policy Template Cheatsheet

The current seller flow uses three first-party policy templates.

## `targeted_request`

Use when:

- access is narrow and request-scoped
- export should stay off
- the asset is mainly being vended to a named requester

Current posture:

- tool tags include `openagents.data_market` and `nostr.nip90`
- export disabled
- derived outputs disabled
- short retention window

## `evaluation_window`

Use when:

- the buyer needs a bounded preview or analysis window
- derived outputs are allowed, but export should stay off
- the seller wants tighter bundle-size and retention bounds

Current posture:

- export disabled
- derived outputs allowed
- shorter retention and bundle limits than a broad bundle license

## `licensed_bundle`

Use when:

- the seller intends an explicit bundle delivery
- the buyer is getting a clearer licensed package rather than only a bounded
  evaluation surface
- delivery and post-sale lifecycle need to be explicit

Current posture:

- includes bundle-delivery scope
- supports the clearer sold-bundle framing
- still depends on explicit delivery/read-back truth

## Reminder

- Pick the narrowest template that matches the promised access.
- Do not widen permissions beyond what the seller explicitly wants.
- Preview the grant before publish so the effective template and timing are
  visible in seller truth.
