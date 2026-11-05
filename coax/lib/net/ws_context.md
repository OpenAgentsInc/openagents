# Observed WebSocket opening context

`ws_context.coax` adds bounded, atomic field/request writers. It performs no
network effects. `relay_conn_open_context` consumes the observed values;
`relay_conn_open` preserves the context-free interface.

- `ws_context_admit(origin, cookie) -> bool` admits the world's explicitly
  bound, already serialized platform Origin and the platform cookie jar's
  header value for the actual target URL. Origin is at most2048 visible ASCII
  bytes without spaces; Cookie is at most8192 visible ASCII bytes including
  spaces. Controls, DEL and non-ASCII refuse. `null` is preserved. Both empty
  values retain a context-free legacy request; cookies require a bound Origin.
- `ws_context_sizes(origin_bytes, cookie_bytes) -> bool` admits nonnegative
  observed UTF-8 extents with those same bounds before retaining a copy.
- `ws_context_fields(origin, cookie, out[]) -> int` writes exactly `Origin`
  and, when present, `Cookie`, each with CRLF. It requires at most10260 bytes.
- `ws_request_context(host, port, path, key, origin, cookie, out[]) -> int`
  retains the existing checked `ws_request` bytes and inserts those fields
  before the final empty line. An empty context is byte-identical to the
  existing request. Every refusal returns-1 without touching output.

This field admission does not parse or authorize an Origin URL, select
cookies, invent an identity, or grant a target access to another jar entry.
The host must obtain Origin from its world configuration and cookies from
the concrete target's platform jar, then retain their bytes with that
specific connection owner. The socket import cannot supply either field.
URI admission, TLS trust and response/upgrade admission remain their existing
authorities. HTTP redirects and authenticated remote behavior need actual
adapter fixtures separately.

The new root gate kills77/77 mutants without equivalent annotations. It
covers exact field bytes, empty context, each byte/capacity boundary, header
injection and atomic refusal. Actual native adapter receipts are described in
the Metal host verification record; this does not alter a renderer default.
