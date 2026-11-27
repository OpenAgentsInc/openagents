# `coax/lib/mcp/` — Native Coax Model Context Protocol Library

Pure `#authority` Sans-I/O MCP protocol foundations supporting both **`2025-11-25`** (stateful) and **`2026-07-28`** (stateless GA), designed per [docs/mcp/coax-architecture-and-plan.md](../../docs/mcp/coax-architecture-and-plan.md).

## Phase 0 Modules (Pure `#authority`, Zero Heap)

| File | Responsibility |
| --- | --- |
| `version.coax` | Revision constants (`2024-11-05`..`2026-07-28`), `mcp_version_has_initialize`, legacy/modern negotiation, `_meta` & extension ID grammar |
| `jsonrpc.coax` | JSON-RPC 2.0 envelope classifier & writer, batch refusal, duplicate-key refusal, all standard & MCP error codes (`-32700..-32042`) |
| `uri_template.coax` | General RFC 3986 URI parser (`file://`, `memo://`, loopback, IPv4/IPv6), RFC 8707 canonicalizer & PRM resource match guard, RFC 6570 Level 1–3 template expander & reverse matcher |
| `headers.coax` | `2026-07-28` SEP-2243 `Mcp-Method`/`Mcp-Name`/`Mcp-Param-*` helpers, `=?base64?...?=` sentinel codec, numeric integer validation, and `Origin` DNS-rebinding guard |
| `json_schema.coax` | Bounded JSON Schema Draft 2020-12 validator (preserving `$schema`/`$defs`/`$ref` verbatim) + SEP-2243 `x-mcp-header` static analyzer & argument extractor |
| `auth_policy.coax` | OAuth 2.1 `WWW-Authenticate` parser/emitter, RFC 9728 PRM & RFC 8414/OIDC discovery URL sequences, metadata validation (`issuer`, PKCE `S256`), CIMD/DCR priority, RFC 9207 exact `iss` verification, and `403` scope step-up union |

## Verification

Run unit/property tests and mutation checks on every module:

```bash
for f in version jsonrpc uri_template headers json_schema auth_policy; do
  ./coax/bin/coax test coax/lib/mcp/$f.coax --library
  ./coax/bin/coax mutate coax/lib/mcp/$f.coax --library
done
```
