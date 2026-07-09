# oa-workroomd Managed Preview Ingress

Status: Cloud MVP scaffold for `CND-015`

`oa-workroomd ingress` models the managed preview ingress state for a workroom.
The MVP is intentionally file-backed so Autopilot and Forge adapters can inspect
the same non-secret policy before the service grows an HTTP or Unix-socket API.

```bash
oa-workroomd ingress status --json
oa-workroomd ingress set \
  --visibility public \
  --preview-url https://preview.example.invalid/workroom.local.echo \
  --custom-domain preview.example.invalid \
  --json
oa-workroomd ingress collaborator grant --identity github:OpenAgentsInc/alice --json
oa-workroomd ingress token mint --label preview-collaborator --json
oa-workroomd ingress revoke --target public --json
```

The state file lives at:

```text
ingress-state.json
```

It records:

- visibility: `private`, `collaborators`, or `public`;
- optional managed preview URL;
- optional custom domain;
- named collaborator grants;
- endpoint token digests;
- ingress receipts.

Workrooms start private. Switching to collaborator or public visibility emits a
`preview_exposed` receipt. Named collaborator grants emit a
`collaborator_granted` receipt. Endpoint token mints store only a `sha256:`
digest and emit an `endpoint_token_minted` receipt. Revocations remove matching
token digests, collaborator grants, custom domains, or public/collaborator
visibility and emit an `ingress_revoked` receipt.

All ingress inputs and receipts are validated against the same raw secret,
token, wallet, private key, and private-topology marker filter used by the
metadata and gateway scaffolds.
