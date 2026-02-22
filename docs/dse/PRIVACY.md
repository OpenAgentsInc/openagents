# Privacy Presets

Canonical privacy presets for execution and replay publication.

## Stable Preset Names

- `open_source`
- `private_repo`
- `paranoid`

## Default Intent

1. `open_source`: minimal redaction.
2. `private_repo`: paths/secret-aware redaction with stricter limits.
3. `paranoid`: maximal redaction and minimal allowed operations.

## Enforcement Notes

1. Implementations may be stricter than preset defaults.
2. Loosening defaults requires ADR review.
3. Publication/export must follow replay/privacy rules in `docs/execution/REPLAY.md`.
