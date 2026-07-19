# Tokyo Night palette provenance

- Upstream: <https://github.com/tokyo-night/tokyo-night-vscode-theme>
- Source commit: `7c0f11eaef322f293621ca7befe462214b7ea468`
- Source file: `themes/tokyo-night-color-theme.json`
- Upstream theme/version commit date: 2025-02-05
- License: MIT; retained in `LICENSE.txt` beside this file.

OpenAgents does not execute or import the upstream VS Code theme contribution.
`src/ide/tokyo-night-theme.ts` is an owned, schema-validated, data-only semantic
projection. It maps the pinned palette into Effect Native, Monaco, Pierre,
terminal, Problems, Output, debug, review, proposal, browser, and status roles.

Accessibility adjustment: upstream faint metadata `#787c99` is lifted to
`#8990ad` so 12px normal text clears WCAG AA against `#1a1b26`. The adjustment
is declared in the projection and contrast-tested. No raw `unsafeCSS`, remote
theme fetch, VS Code contribution, command, or executable code is accepted.
