# Security & Privacy — Tinyvex

Threat Model (MVP)

- Local macOS host with trusted iOS/macOS clients on the same LAN or loopback.
- No secrets stored by the library by default; the app is responsible for persistence (Keychain) if needed.

Transport

- Start plaintext on loopback for development.
- For LAN, add optional TLS (wss) and pairing tokens as a follow‑up. Consider mTLS for device binding.

Auth

- Optional `setAuthToken` API; token forwarded to server and applied to the connection.
- No token storage in the library by default.

Input Validation

- JSON decoding must never `try!`; invalid payloads surface as recoverable errors.
- Subscription `name`/`params` are application‑defined; validate against an allowlist to avoid unexpected queries.

Logging

- Avoid logging payloads with PII by default; use redaction helpers and sampling.

