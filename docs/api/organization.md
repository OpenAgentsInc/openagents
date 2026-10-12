# Organization administration

2026-10-12. Members, invitations, bearer keys, and the caps on those keys are live. Customer-managed keys are proposed.

An organization is a workspace whose `kind` is `organization`. You administer it here: the people in it, the bearer keys they call with, the caps you set on those keys, and the encryption key that wraps the workspace's stored data. A personal workspace holds its owner alone. Invitations, role changes, seat changes, and transfers on a personal workspace return `400` `personal_workspace`.

It is a PUBLIC surface on `https://api.openagents.com/v1` (alias `https://openagents.com/api/v1`). It follows [the API design](design.md): `oak_` keys, sats on spend, and no house rate limit on your own keys. Headers, the workspace, list cursors, and body size that every call shares are in [Calling the API](calling.md). Words for the objects on this page are in the [glossary](../glossary.md#decision-service-identity-and-accounting). Administration stays on `/v1/workspaces/{ws}`.

## Conventions

| Topic | Rule |
| --- | --- |
| Auth | `Authorization: Bearer oak_<id>.<secret>` or `Authorization: Bearer sess_<hex>`. |
| Scope | `workspace` for members, invitations, and encryption keys. `keys` for bearer keys and their caps. A key with neither scope is refused on that route. |
| Workspace | The `{ws}` in the path. Another workspace's id is `404` `unknown_workspace`. |
| Errors | `{"error": {"code", "message"}}`. A bad field may also set `param`. Codes are in [Errors](#errors). |
| One owner | The workspace has exactly one owner. Ownership moves only through transfer. An invitation cannot grant `owner`. |

## Roles

| Action | Owner | Admin | Member |
| --- | --- | --- | --- |
| Read the workspace and its members | Yes | Yes | Yes |
| Invite someone, or revoke an invitation | Yes | Yes | — |
| Remove a member | Any other member | Members only | Yourself |
| Change a role between `admin` and `member` | Yes | — | — |
| Rename, set seats, transfer ownership | Yes | — | — |
| Read bearer keys | Every key | Every key | Your own keys |
| Issue, pause, resume, rotate, copy, or revoke a bearer key | Any key | Any key | Your own keys |
| Register or attach an encryption key | Yes | — | — |
| Read encryption keys | Yes | Yes | — |

## Create and read

`POST /v1/workspaces` opens an organization workspace you own.

```json
{ "name": "Production", "seats": 20 }
```

`seats` is optional. Omit it, or send `null`, for no seat cap. The response is the workspace, with your role `owner`. When the service is not accepting new organizations, the call returns `403` `signup_disabled`.

`GET /v1/workspaces/{ws}` returns the workspace, every membership (including `revoked`), and your role. An owner or admin also receives the workspace's invitations.

```json
{
  "workspace": {
    "id": "ws_a1b2c3d4e5f60718",
    "name": "Production",
    "kind": "organization",
    "seats": 20,
    "members_epoch": 4
  },
  "role": "admin",
  "members": [
    {
      "account": "acct_b1c2d3e4f5060718",
      "role": "owner",
      "status": "active",
      "granted": "2026-10-01T15:00:00Z"
    }
  ]
}
```

`members_epoch` increases when membership changes. Compare it with the value you stored to see that your list is stale.

`PATCH /v1/workspaces/{ws}` takes `name`, `seats`, or both. The owner sends it. `seats: null` clears the cap. A cap below the current members plus pending invitations is `409` `seats_below_members`. An empty `name` is `400` `empty_field`.

## Invitations

`POST /v1/workspaces/{ws}/invitations` takes `role` (`admin` or `member`) and an optional `ttl_secs`. The default lifetime is 7 days. The maximum is 30 days. The response includes `token` once. You deliver that token. The store keeps none of it.

```json
{
  "invitation": {
    "id": "inv_c1d2e3f405162738",
    "workspace": "ws_a1b2c3d4e5f60718",
    "role": "member",
    "expires_unix": 1760000000
  },
  "token": "inv_inv_c1d2e3f405162738.secret"
}
```

The token is `inv_`, then the invitation id, then `.` and the secret. The id already starts with `inv_`, so the token starts with `inv_inv_`. `role: "owner"` is `400` `owner_by_invitation`. A full seat list is `409` `seat_limit`.

`DELETE /v1/workspaces/{ws}/invitations/{id}` revokes a pending invitation. The record stays, with `status` `revoked`.

`POST /v1/invitations/accept` with `{"token": "inv_..."}` joins the signed-in account. A second use of the same token is `409` `invitation_closed`. An expired token is `403` `invitation_expired`. A token that does not match is `403` `invalid_invitation`. The account is already a member: `409` `already_member`.

`POST /v1/invitations/accept-reviewed` takes `token`, `workspace`, and `role` (`admin` or `member`). The call joins only when those three match the invitation.

## Members

`PATCH /v1/workspaces/{ws}/members/{account}` takes `{"role": "admin"}` or `{"role": "member"}`. The owner sends it. Changing the owner on this route is `400` `ownership_by_transfer`.

`DELETE /v1/workspaces/{ws}/members/{account}` removes that member. The membership stays with `status` `revoked`, their sessions end, and their bearer keys fail on the next call. Removing the owner is `409` `last_owner`. An admin cannot remove the owner or another admin.

`POST /v1/workspaces/{ws}/transfer` takes `{"account": "acct_..."}`. The owner becomes an admin, and that account becomes the owner, in one change. The target is an active member.

`GET /v1/workspaces/{ws}/access` says what the caller may do in the workspace.

## Bearer keys

A bearer key is `oak_<id>.<secret>`. It authenticates calls. It is not an encryption key, and it is not a [provider key](../glossary.md) (the workspace's own OpenRouter or Vercel credential).

| Method | Path | What it does |
| --- | --- | --- |
| `GET` | `/v1/workspaces/{ws}/keys` | List keys. A member sees their own. An owner or admin sees every key in the workspace. |
| `POST` | `/v1/workspaces/{ws}/keys` | Issue a key. The body is `{"name": "ci", "scopes": ["responses", "models:read"]}`. `key_token` is returned once. |
| `POST` | `/v1/workspaces/{ws}/keys/{key}/pause` | Pause a key. Its calls are refused until resume. |
| `POST` | `/v1/workspaces/{ws}/keys/{key}/resume` | Resume a paused key. |
| `POST` | `/v1/workspaces/{ws}/keys/{key}/rotate` | Replace the secret. The new `key_token` is returned once. |
| `POST` | `/v1/workspaces/{ws}/keys/{key}/copy` | Issue a new key with the same scopes and caps. The secret is returned once. |
| `DELETE` | `/v1/workspaces/{ws}/keys/{key}` | Revoke a key. The record stays, with `status` `revoked`. |

A key made without `scopes` receives `responses`, `models:read`, and `usage:read`. The scope names are in [the API design](design.md). Status is `active`, `paused`, or `revoked`. A missing id is `404` `unknown_key`. A key from another workspace is `403` `tenant_mismatch`.

## Caps you set

The caps on a key are the ones the workspace set. You read them here.

`GET` and `PUT /v1/workspaces/{ws}/keys/{key}/limits` read and replace them. An owner or admin may change any key in the workspace. A member may change their own.

```json
{
  "spend_cap": { "usd": "25.00", "period": "month" },
  "max_price": { "input": "5.00", "output": "25.00" },
  "models": ["openagents/fast"],
  "requests_per_minute": 60,
  "expires_at": 1762000000
}
```

`period` is `day`, `month`, or `total`. Dollar amounts are decimal strings. A cap you hit returns `403` `limit_reached` or `429` with `Retry-After`, as [the API design](design.md) describes.

Spend for the workspace is `GET /v1/workspaces/{ws}/usage`, with activity, a time series, and an export on the paths under it. Those reads are already on the account service.

## Customer-managed keys

Proposed. These routes are not mounted.

A customer-managed key is an encryption key your organization holds in its own key service. OpenAgents calls that service to wrap and unwrap data keys. The raw key never arrives here, and a response never contains it. Registering one does not change who pays for model calls. A [provider key](../byok/README.md) does that. It also does not seal a person's own files: those stay on `openagents vault`, specified in [NIP-VAULT](../../nips/openagents/NIP-VAULT.md). A session's credential vault (`vlt_`) stays the object in [Managed Agents](managed-agents.md#vaults).

The key wraps three stores once it is attached: workspace files (`file_`), stored model responses (`store: true`), and files a cloud session writes. The model still receives the prompt for the route the call selected. Attaching a key does not make a route private.

One organization workspace has one attached key. A personal workspace cannot register one (`400` `personal_workspace`).

| Method | Path | Who | What it does |
| --- | --- | --- | --- |
| `POST` | `/v1/workspaces/{ws}/encryption-keys` | Owner | Register a key. Status starts at `pending`. |
| `GET` | `/v1/workspaces/{ws}/encryption-keys` | Owner, admin | List keys. |
| `GET` | `/v1/workspaces/{ws}/encryption-keys/{id}` | Owner, admin | Read one key (`cmk_`). |
| `POST` | `/v1/workspaces/{ws}/encryption-keys/{id}/validate` | Owner | Wrap and unwrap a throwaway data key, then discard it. |
| `POST` | `/v1/workspaces/{ws}/encryption-keys/{id}/attach` | Owner | Use this key for new writes. |
| `POST` | `/v1/workspaces/{ws}/encryption-keys/{id}/disable` | Owner | Stop using the key. Wrapped reads and writes fail until you attach a valid key again. |

```json
{
  "name": "production",
  "region": "us",
  "provider": {
    "type": "aws_kms",
    "key_arn": "arn:aws:kms:us-east-1:123456789012:key/abcd"
  }
}
```

`region` is `us` or `eu`, the region where OpenAgents may call the key service. `provider.type` is `aws_kms` with `key_arn`, or `gcp_kms` with `key_name` set to the full Cloud KMS resource name. Any other shape is `400` `invalid_provider`. A key service in a different region from `region` is `400` `region_mismatch`.

Validate moves a successful key to `valid`. A failed round trip leaves it `pending` and returns `409` `validation_failed`, with `param` naming the provider field that failed. Attach requires `valid`. Attaching a `pending` key is `409` `key_not_valid`. Attach sets the workspace's `encryption_key_id` and the key's status to `active`. Bytes already stored stay wrapped by the previous key until something writes them again. The previous key's record stays `valid`, so those reads still succeed.

Disable sets the key to `disabled`. A read or write that needs a disabled key returns `409` `encryption_key_disabled`. Membership, invitations, and bearer keys keep working. A missing id is `404` `encryption_key_not_found`.

`GET /v1/workspaces/{ws}` gains `encryption_key_id` when a key is attached. The field is null when none is.

## Errors

| Status | `code` | When |
| --- | --- | --- |
| `400` | `personal_workspace` | The workspace is personal. |
| `400` | `owner_by_invitation`, `ownership_by_transfer`, `empty_field`, `invalid_request`, `malformed_invitation` | The body cannot be applied. |
| `400` | `invalid_provider`, `region_mismatch` | The encryption key's provider block. Proposed. |
| `401` | `unauthenticated` | The bearer is missing or no longer valid. |
| `403` | `not_member`, `membership_revoked`, `forbidden`, `signup_disabled` | The caller cannot do this. |
| `403` | `invalid_invitation`, `invitation_expired`, `tenant_mismatch` | The token or the key is not for this workspace. |
| `404` | `unknown_workspace`, `unknown_account`, `unknown_key` | That id is not in this workspace. |
| `404` | `encryption_key_not_found` | Proposed. |
| `409` | `seat_limit`, `seats_below_members`, `last_owner`, `already_member`, `invitation_closed` | The membership change conflicts with the workspace as it stands. |
| `409` | `key_not_valid`, `validation_failed`, `encryption_key_disabled` | Proposed. |
| `503` | `accounts_unavailable` | The account store cannot be read or written. Retry. |

## What this page leaves out

Single sign-on is a separate built surface at `/v1/workspaces/{ws}/sso`, listed in [the API design](design.md). Workload identity for a continuous-integration job is not specified here. Team policy is built and not offered. A shell, memory, and skills run in a [session](managed-agents.md), not on this page.
