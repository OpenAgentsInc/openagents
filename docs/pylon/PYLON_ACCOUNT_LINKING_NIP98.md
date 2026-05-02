# Pylon Account Linking NIP-98 Proof

`pylon account link` now carries two layers:

- the existing short-lived one-time token in the JSON body
- a NIP-98 `Authorization` header signed by the node identity

That keeps the pragmatic dashboard link flow intact while binding the request to
the actual Pylon key.

## Request Shape

- `POST /api/pylon-links/complete`
- `Content-Type: application/json`
- `Authorization: Nostr <base64-encoded signed event json>`

The JSON body still carries:

- `token`
- `public_key_hex`
- `npub`
- `node_label`
- `runtime_state`
- `runtime`
- `ready_model`
- `eligible_product_count`
- `products`

`runtime` is a nested diagnostic summary copied from the local provider status
snapshot. It may include `mode`, `last_action`, `last_error`,
`degraded_reason_code`, `authoritative_status`, `authoritative_error_class`,
`execution_backend_label`, and `provider_blocker_codes`. The website stores
those fields for linked-node diagnostics so an operator sees the actual local
blocker instead of a bare `Error` label.

The NIP-98 event binds:

- exact absolute URL
- exact HTTP method (`POST`)
- SHA-256 hash of the raw JSON request bytes

## Website Verification Contract

The website can verify the proof with the current Rust implementation by:

1. Decoding the `Authorization` header as a NIP-98 event.
2. Verifying the Nostr event signature and event id.
3. Validating the event against the exact absolute completion URL.
4. Validating the method is `POST`.
5. Recomputing the SHA-256 hash of the raw request body bytes and matching the
   NIP-98 `payload` tag.
6. Requiring the verified event `pubkey` to equal the body `public_key_hex`.
7. Keeping the one-time token requirement and expiry/consumption checks.

This means a stolen bearer token alone is no longer enough if the website
starts enforcing the NIP-98 layer.

## Current OpenAgents CLI Behavior

The CLI now:

- serializes the JSON body first
- hashes those exact bytes for NIP-98 payload binding
- signs the request with the local Pylon identity
- ignores a live admin status endpoint if it does not explicitly report the
  same public key as the local identity used to sign the account-link proof
- includes the current runtime diagnostic summary in the signed JSON body
- sends the NIP-98 header on the same completion request
- exposes proof metadata in the command report:
  - `proof_scheme`
  - `proof_event_id`
  - `proof_payload_hash`

## Important Product Posture

- Local bring-up is still complete without any account linking.
- Account linking stays optional.
- The signed proof hardens the optional web-account flow; it does not make
  login or linking part of default onboarding.
