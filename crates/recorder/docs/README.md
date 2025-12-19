# Recorder

Recorder is a line-based flight recorder format for agent sessions. This crate provides a parser, validator, and CLI utilities for `.rlog` files.

## Scope (Current)

- Parser and validator for the `.rlog` format.
- CLI for validation, stats, parsing, and step renumbering.
- Optional database export (feature-flagged).
- UI components live in `ui::recorder` and are surfaced in Storybook at `/stories/recorder/*`.

## CLI

Install or run from the workspace:

```bash
cargo run -p recorder -- --help
```

### Validate

```bash
recorder validate path/to/session.rlog
recorder validate path/to/session.rlog --verbose
recorder validate path/to/session.rlog --format json
```

### Stats

```bash
recorder stats path/to/session.rlog
```

### Parse

```bash
recorder parse path/to/session.rlog
recorder parse path/to/session.rlog --lines --max-lines 100
```

### Fix (renumber steps)

```bash
recorder fix path/to/session.rlog --renumber-steps
recorder fix path/to/session.rlog --renumber-steps --write
recorder fix path/to/session.rlog --renumber-steps --output fixed.rlog
```

### Export (feature: `export`)

```bash
cargo run -p recorder --features export -- export --help
```

## References

- Format spec: `crates/recorder/docs/format.md`
- UI components: `crates/ui/src/recorder/`
- Storybook: `cargo storybook` then visit `/stories/recorder`
