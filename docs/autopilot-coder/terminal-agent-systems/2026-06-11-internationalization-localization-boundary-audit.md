# Internationalization And Localization Boundary Audit

Date: 2026-06-11

This is system #61 from the Bun/Effect terminal-agent systems list. It defines
where localization may apply in the terminal agent and where protocol,
receipt, policy, code, and diagnostic identifiers must remain stable.

## Target

Build an internationalization boundary that supports localized UI text,
help, dates, numbers, and user-facing errors while keeping machine-readable
schemas, refs, policies, and receipts language-stable.

## User-Visible Capability

Users should be able to:

- Select or inherit a locale.
- Read help, status, warnings, and errors in a supported language.
- Use localized date, time, number, and currency formatting.
- Keep commands, refs, and JSON output stable.
- Fall back cleanly when a translation is missing.

Localization should not change runtime semantics.

## Boundary Model

Localizable:

- UI labels.
- Help text.
- Error explanations.
- Tip content.
- Release-note summaries.
- Date, time, number, and currency presentation.

Not localized:

- Schema keys.
- Event kinds.
- Capability refs.
- Policy refs.
- Receipt refs.
- Command ids.
- Tool names.
- JSON output fields.
- Permission ids.

## Bun/Effect Boundary

Use Effect services for:

- `LocalePreferenceService`: resolves user, team, and system locale.
- `MessageCatalogService`: loads and validates localized messages.
- `FormatterService`: formats dates, numbers, durations, and currency.
- `LocalizationBoundaryService`: prevents stable ids from being translated.

Use Schema for message catalogs and missing-translation reports.

## Safety Rules

- A localized string cannot become a machine identifier.
- Permission prompts must preserve exact action and policy refs.
- Payment, payout, and settlement language must remain precise.
- Missing translations fall back visibly, not silently to incorrect copy.
- Model prompts that rely on stable ids should use canonical ids.
- Public receipts stay language-stable.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents public and terminal-agent docs are written in
English and the terminal-agent README does not yet include an i18n/localization
boundary audit.

Related anchors:

- #4773 API parity contract because API fields must remain stable.
- #4772 MVP exit review because public copy and product claims need exact
  language.
- #4785 settlement visibility law because settlement refs must be stable.

No localization claim should be green until message catalogs, stable-id
boundaries, fallback behavior, and payment language review exist.

## Tests

Minimum coverage:

- Resolve locale from user and environment preferences.
- Render localized UI strings.
- Preserve schema keys and refs across locales.
- Show fallback for missing translations.
- Format dates, durations, numbers, and currency.
- Verify permission prompts keep canonical action refs.
- Validate translation catalogs.
- Reject translation of command ids and receipt refs.

## Decision

Localization should improve readability without changing the runtime contract.
Stable ids stay stable; human-facing explanation can be translated around
them.

