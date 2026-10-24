# Separate surface preference from theme selection

> **Deprecated (owner, 2026-10-24).** All decision records outside [`docs/adr/`](../../../../docs/adr/README.md) are deprecated; see [ADR 0001](../../../../docs/adr/0001-one-decision-record-folder.md). This record is history.

OpenAgents treats glass as an independent, device-local surface preference instead of a property that changes implicitly with theme selection. Each theme variant retains a recommended surface treatment (`Frosted` for OpenAgents and `Opaque` for VS Code-derived themes), while the user may preserve that recommendation or globally force `Frosted` or `Opaque`; forced frost derives its tints from the selected theme's own shell and background roles, and unsupported window compositors remain opaque.

This preserves theme authors' contrast assumptions by default while allowing users to keep a consistent glass preference across every built-in and custom theme. Syntax, terminal, semantic, status, diff, and accent roles are unaffected by surface preference.
