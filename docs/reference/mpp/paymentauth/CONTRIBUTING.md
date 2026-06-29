# Contributing

## Making Changes

### Branching

- `main` is the stable branch
- Create feature branches for changes: `git checkout -b add-lightning-method`

### Pull Request Checklist

Before submitting a PR:

1. **Build succeeds**: `make check` passes
2. **Lint passes**: `make lint` passes
3. **Version bumped**: If modifying a spec, increment the version number in the filename and frontmatter
4. **References are stable**: Avoid new hardcoded external references like `Section X.Y of {{I-D...}}`

### Types of Changes

| Change Type | Process |
|-------------|---------|
| Typo/editorial fix | Direct PR to `main` |
| New intent | Use [intent template](examples/intent-template.md), add to `specs/intents/` |
| New method | Use [method template](examples/method-template.md), add to `specs/methods/` |
| New extension | Use [extension template](examples/extension-template.md), add to `specs/extensions/` |
| New method with experimental intent | Define intent in method spec, add to `specs/methods/` |
| Core protocol change | Open an issue first for discussion |

### Experimental Intents

Methods may define new intent types that are not yet formalized in `specs/intents/`. These are considered **experimental intents**.

| Intent Location | Status | Requirements |
|-----------------|--------|--------------|
| `specs/intents/` | Standardized | Adopted by 2+ methods |
| `specs/methods/` only | Experimental | Single method definition |

**Workflow for new intents:**

1. **Propose with method**: Define the intent semantics directly in your method spec (`specs/methods/draft-{network}-{intent}-00.md`). The intent is automatically experimental.

2. **Gain adoption**: Other methods implement the same intent pattern in their own specs.

3. **Formalize**: Once 2+ methods implement the intent, extract common semantics into `specs/intents/draft-payment-intent-{name}-00.md`.

**Example**: A session payment method might define:

```
specs/methods/draft-tempo-session-00.md     ← defines "session" intent (experimental)
specs/methods/draft-lightning-session-00.md ← second implementation
specs/intents/draft-payment-intent-session-00.md ← formalized after adoption
```

This ensures intents are battle-tested before standardization, preventing premature abstractions.

### Naming Conventions

| Type | Pattern | Example |
|------|---------|---------|
| Core | `draft-httpauth-payment-XX` | `draft-httpauth-payment-00` |
| Intent | `draft-payment-intent-{name}-XX` | `draft-payment-intent-charge-00` |
| Method | `draft-{network}-payment-method-XX` | `draft-tempo-payment-method-00` |
| Extension | `draft-payment-{feature}-XX` | `draft-payment-discovery-00` |

## Writing Style

See [STYLE.md](STYLE.md) for RFC writing conventions and design principles.

## AI-Assisted Contributions

If you use AI tools to help write specifications or code:

1. You are responsible for the correctness and quality of the output
2. Disclose significant AI assistance in your PR description
3. Review all generated content for RFC compliance

### Agent Skills

This repo ships [Amp](https://ampcode.com) agent skills in `.agents/skills/`. Skills are loaded automatically when working in this repo. To install them globally (so they're available across all projects):

```bash
./scripts/install-skills.sh
```

## Building the Specifications

### Using Docker (recommended)

```bash
# Build the Docker image (first time only)
make docker-build

# Generate XML, HTML, TXT, and PDF artifacts
make build

# Run build + validation
make check

# Interactive shell for debugging
make shell

# Clean generated artifacts
make clean
```

### Local Development

```bash
# Install dependencies
bundle install              # Ruby: kramdown-rfc
pip install -r requirements.txt  # Python: xml2rfc, rfclint

# Generate artifacts
./scripts/gen.sh

# With verbose output
./scripts/gen.sh --verbose
```

### Generated Outputs

All outputs are written to `artifacts/`:

- `draft-*.xml` - RFC XML (v3)
- `draft-*.html` - HTML version
- `draft-*.txt` - Plain text version
- `draft-*.pdf` - PDF version
