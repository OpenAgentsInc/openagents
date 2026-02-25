# Route Split Domain Rollback Matrix (OA-WEBPARITY-013)

Canonical per-domain rollback mapping used by Rust control service route-split controls.

## Route Groups

- `auth_entry`
  - prefixes: `/login`, `/register`, `/authenticate`, `/onboarding/*`
  - rollback target: `legacy`
  - HTMX default mode: `fragment`
  - HTMX rollback mode: `full_page`
- `account_settings_admin`
  - prefixes: `/account/*`, `/settings/*`, `/admin/*`
  - rollback target: `legacy`
  - HTMX default mode: `fragment`
  - HTMX rollback mode: `full_page`
- `billing_l402`
  - prefixes: `/billing/*`, `/l402/*`
  - rollback target: `legacy`
  - HTMX default mode: `fragment`
  - HTMX rollback mode: `full_page`
- `chat_pilot`
  - prefixes: `/chat/*`, `/feed`, `/`
  - rollback target: `rust_shell`
  - HTMX default mode: `fragment`
  - HTMX rollback mode: `full_page`

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

HTMX mode per-domain override:

```json
{"target":"htmx_full_page","domain":"chat_pilot"}
```

HTMX mode rollback target application:

```json
{"target":"htmx_rollback","domain":"chat_pilot"}
```

HTMX mode clear:

```json
{"target":"htmx_clear","domain":"chat_pilot"}
```

## Operational Notes

- Domain overrides are in-memory runtime controls and are visible via `GET /api/v1/control/route-split/status`.
- Codex worker control API paths remain pinned to `rust_shell` authority regardless of global/domain override (`/api/runtime/codex/workers*`).
- HTMX targets (`htmx_fragment`, `htmx_full_page`, `htmx_rollback`, `htmx_clear`) require `domain` so route groups can be rolled out independently.
