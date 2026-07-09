# oa-workroomd Link-Local Gateways

Status: Cloud MVP scaffold for `CND-014`

`oa-workroomd gateway` models the link-local capability gateways used inside a
managed workroom.

```bash
oa-workroomd gateway policy init --json
oa-workroomd gateway access --gateway model --capability model.gateway --json
oa-workroomd gateway revoke --capability model.gateway --json
```

The default policy covers:

- `model`
- `artifacts`
- `receipts`
- `memory`
- `email`
- `settlement`

Each gateway has an explicit capability allow-list. Access is allowed only when
the requested capability is listed for that gateway and has not been revoked.
Revocation is file-backed, so the next access check sees it without restarting
the process.

Every access writes a redacted audit event to:

```text
gateway-access.jsonl
```

The audit event records gateway, capability, decision, and reason. Policy and
audit fields are validated against the same raw secret, token, wallet, key, and
private-topology marker filter used by the metadata endpoint.
