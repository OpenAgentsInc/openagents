# Route Split Domain Rollback Matrix (OA-WEBPARITY-013)

Canonical per-domain rollback mapping used by Rust control service route-split controls.

## Route Groups

- `auth_entry`
  - prefixes: `/login`, `/register`, `/authenticate`, `/onboarding/*`
  - rollback target: `legacy`
- `account_settings_admin`
  - prefixes: `/account/*`, `/settings/*`, `/admin/*`
  - rollback target: `legacy`
- `billing_l402`
  - prefixes: `/billing/*`, `/l402/*`
  - rollback target: `legacy`
- `chat_pilot`
  - prefixes: `/chat/*`, `/feed`, `/`
  - rollback target: `rust_shell`

## Control API Usage

Global override:

```json
{"target":"legacy"}
```

Global clear:

```json
{"target":"clear"}
```

Per-domain override:

```json
{"target":"legacy","domain":"billing_l402"}
```

Per-domain rollback target application:

```json
{"target":"rollback","domain":"billing_l402"}
```

Per-domain clear:

```json
{"target":"clear","domain":"billing_l402"}
```

## Operational Notes

- Domain overrides are in-memory runtime controls and are visible via `GET /api/v1/control/route-split/status`.
- Codex worker control API paths remain pinned to `legacy` authority regardless of global/domain override (`/api/runtime/codex/workers*`).
