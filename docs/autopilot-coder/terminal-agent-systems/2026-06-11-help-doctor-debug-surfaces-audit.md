# Help, Doctor, And Debug Surfaces Audit

Date: 2026-06-11

This is system #27 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should expose help, command discovery, environment
diagnostics, support bundles, performance diagnostics, logs, and user-safe
debug exports.

## Target

Build support surfaces that help users and operators diagnose problems without
leaking private workspace data.

Help should explain available capabilities. Doctor should validate the local
environment. Debug surfaces should collect enough evidence to investigate a
failure while applying strict redaction, consent, and public/private boundaries.

## User-Visible Capability

The user should be able to:

- Open a help panel with general guidance and command listings.
- Browse available built-in and custom commands.
- See unavailable or disabled capabilities where appropriate.
- Run a doctor check for installation, settings, shell, search, sandbox,
  extension, and integration problems.
- See actionable warnings and fixes.
- Run preflight connectivity checks.
- Export a redacted support bundle.
- Submit feedback or issue reports with explicit consent.
- Capture performance diagnostics for slow startup or long operations.
- Capture memory diagnostics when explicitly requested.

Support actions should be clear about what data they collect before submission.

## Help Surface

The help surface should include:

- General usage guidance.
- Command browser.
- Shortcut hints.
- Version and channel information.
- Links to docs.
- Custom command section.
- Empty-state handling for missing custom commands.
- Dismiss and exit handling.

Help should render from the live command registry so disabled, hidden, and
policy-blocked commands are represented correctly.

## Doctor Surface

Doctor should produce a structured diagnostic report with:

- Install type.
- App version.
- Executable location class.
- Package manager or updater owner.
- Update permissions.
- Search backend status.
- Multiple-installation warnings.
- Shell configuration warnings.
- Settings validation errors.
- Keybinding conflicts.
- Integration parsing warnings.
- Sandbox support.
- Extension load errors.
- Agent definition errors.
- Environment variable validation.
- Version lock or update lock status.
- Connectivity status.
- Context usage warnings.

The UI should group results by severity and offer short fixes. The machine
report should be schema-validated and safe to redact.

## Debug And Support Bundles

Support bundles should be opt-in and typed.

Potential bundle sections:

- User-written description.
- Public-safe environment summary.
- Version and platform summary.
- Git presence, not raw repository content.
- Recent error summaries.
- Last API request metadata with private fields removed.
- Current transcript after normalization and redaction.
- Background task transcript summaries where policy allows.
- Diagnostic report.
- Performance timings.
- Memory dump refs, only for explicit local debugging.

Before upload or issue creation, the user should be asked to consent to the
data classes being included.

## Redaction Rules

The bundle builder should redact:

- API keys.
- Bearer tokens.
- Authorization headers.
- Cloud credentials.
- Password-like values.
- Secret environment variables.
- Private file paths when crossing a public boundary.
- Raw prompts when the target bundle is public-safe.
- Private transcript sections unless the user opts in.

Redaction should be schema-aware where possible and regex-backed for common
secret forms. The system should record that redaction was applied without
claiming it is perfect.

## Diagnostic Logging

Diagnostic logs should support:

- Structured JSON line events.
- Level, event name, timestamp, and duration.
- Startup and slow-operation timings.
- Non-PII metadata only by default.
- Silent failure when the diagnostic path is unavailable.
- Explicit debug mode for more verbose local-only logs.

Public-safe diagnostic logging must not include prompts, file paths, project
names, repository names, or raw command output.

## Core Design

Define a `SupportDiagnosticsService` that owns help data, doctor checks,
support bundle generation, redaction, and diagnostic logging.

Suggested service boundary:

```ts
interface SupportDiagnosticsService {
  help(request: HelpRequest): Effect.Effect<HelpModel, SupportDiagnosticsError>
  doctor(request: DoctorRequest): Effect.Effect<DoctorReport, SupportDiagnosticsError>
  preflight(request: PreflightRequest): Effect.Effect<PreflightReport, SupportDiagnosticsError>
  bundle(request: SupportBundleRequest): Effect.Effect<SupportBundlePlan, SupportDiagnosticsError>
  export(request: SupportExportRequest): Effect.Effect<SupportExportReceipt, SupportDiagnosticsError>
  log(event: DiagnosticEvent): Effect.Effect<void, never>
}
```

Bundle creation should return a plan first, then require consent before upload
or public issue drafting.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for diagnostics, redaction, bundle planning, and export.
- `Schema` for doctor reports, help models, bundle sections, and log events.
- `Layer` for environment, updater, settings, network, git, extension, and
  terminal-capability checkers.
- `Stream` for progressive diagnostic checks.
- `Queue` for support-submission state transitions.
- `Schedule` for connectivity retries and slow-operation timing.
- `Redacted` or equivalent wrappers for sensitive values.

Each checker should be independently testable and allowed to fail without
blocking unrelated checks.

## Safety Rules

- Do not upload support bundles without explicit consent.
- Do not include raw secrets in help, doctor, logs, or support exports.
- Do not include raw prompts or file contents in public-safe bundles by
  default.
- Do not let a failed checker crash the doctor surface.
- Do not make network preflight a hard requirement for offline/local usage
  unless the requested action needs network access.
- Do not write memory diagnostics to public locations unless the user asks and
  the output path is shown.
- Do not treat update availability as proof that local installation is broken.
- Do not include private diagnostic paths in public issue titles or summaries.

## Tests

Minimum regression coverage:

- Render help from the live command registry.
- Group built-in and custom commands.
- Hide commands that are not user-visible.
- Run doctor with passing, warning, and failing checkers.
- Continue doctor when an optional checker throws.
- Validate settings, keybinding, extension, sandbox, and search statuses.
- Run connectivity preflight with success, HTTP failure, DNS failure, and TLS
  hint cases.
- Build a support bundle plan and require consent before export.
- Redact common token, key, header, and password forms.
- Keep public-safe logs free of prompts, paths, project names, and repository
  names.
- Capture timing diagnostics around slow operations.
- Handle memory-diagnostic failure without losing the support flow.

## OpenAgents Translation Notes

When promoted, map help and diagnostics to OpenAgents operator UX, capability
refs, policy refs, support artifact refs, public/private projection boundaries,
and issue-report receipts. Verify live issue state before claiming doctor,
support export, or diagnostic logging behavior is implemented.

## Decision

Help, doctor, and debug surfaces should be one support system with strict data
classification. The agent should make capabilities discoverable, diagnose local
problems, and gather support evidence only through redacted, consented,
public-safe flows.
