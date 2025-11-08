# ACP Lint Guardrails

To protect the ACP wire contract, we add lightweight lint guardrails and guidance.

## Rules (SwiftLint)

- No JSONSerialization in ACP wire types
  - Path: `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/`
  - Rationale: All wire types must use Codable; ad‑hoc JSON risks contract drift.

- CodingKeys or custom encode/decode (hint)
  - Path: `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/`
  - Severity: warning
  - Rationale: Prefer explicit CodingKeys or custom encode/decode for wire payloads.
  - Opt‑out: add `// swiftlint:disable:this acp_codingkeys_hint` on the declaration line if intentional.

## CI

A GitHub Actions workflow runs SwiftLint on pull requests and main pushes. Violations are surfaced in annotations and will fail the job for `error` severity.

## Guidance

- Prefer explicit `CodingKeys` even if property names match JSON keys today.
- Keep casing exactly as the ACP spec requires (snake_case vs camelCase by type).
- Avoid ad‑hoc `JSONSerialization` in wire models; use Codable and custom encode/decode where needed.

