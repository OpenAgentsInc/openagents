# BlackBox

BlackBox is a line-based flight recorder format for agent sessions. This crate provides a parser, validator, and CLI utilities for `.bbox` files.

## Scope (Current)

- Parser and validator for the `.bbox` format.
- CLI for validation, stats, parsing, and step renumbering.
- Optional database export (feature-flagged).
- UI components live in `ui::blackbox` and are surfaced in Storybook at `/stories/blackbox/*`.

## CLI

Install or run from the workspace:

```bash
cargo run -p blackbox -- --help
```

### Validate

```bash
blackbox validate path/to/session.bbox
blackbox validate path/to/session.bbox --verbose
blackbox validate path/to/session.bbox --format json
```

### Stats

```bash
blackbox stats path/to/session.bbox
```

### Parse

```bash
blackbox parse path/to/session.bbox
blackbox parse path/to/session.bbox --lines --max-lines 100
```

### Fix (renumber steps)

```bash
blackbox fix path/to/session.bbox --renumber-steps
blackbox fix path/to/session.bbox --renumber-steps --write
blackbox fix path/to/session.bbox --renumber-steps --output fixed.bbox
```

### Export (feature: `export`)

```bash
cargo run -p blackbox --features export -- export --help
```

## References

- Format spec: `crates/blackbox/docs/format.md`
- UI components: `crates/ui/src/blackbox/`
- Storybook: `cargo storybook` then visit `/stories/blackbox`
