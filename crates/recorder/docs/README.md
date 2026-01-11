# Recorder

Recorder is a line-based flight recorder format for agent sessions. This crate provides a parser, validator, and CLI utilities for `.rlog` files.

See [`format.md`](format.md) for the complete format specification.

## Why This Format?

Existing agent session formats (Codex JSONL, OpenAI conversation exports, etc.) are optimized for their respective platforms but fall short for multi-agent coordination:

1. **Attribution & Accounting** — When multiple agents collaborate on a task, you need deterministic attribution: which agent made which tool calls, how many tokens did each consume, what was the reasoning chain? The rlog format captures timestamped tool traces with call IDs, thinking blocks, subagent spawns, and token accounting—everything needed to reconstruct who did what.

2. **Streaming & Live Coordination** — The line-based format is designed for streaming. Agents can tail each other's logs in real-time to maintain shared context without heavy polling or complex sync protocols. Each line is self-contained and parseable.

3. **Auditability** — Session recordings become verifiable receipts. A cryptographic hash of an rlog file can be referenced externally (e.g., in a payment settlement) to prove what work was done. The format is human-readable for manual audits but structured enough for programmatic verification.

4. **Training Data** — Successful sessions become training data for future agents. The structured format makes it easy to extract patterns: which tool sequences work, how do effective agents reason through problems, what collaboration patterns succeed?

5. **Platform Independence** — By defining our own format, we're not locked into any vendor's schema changes. We can convert from Codex, OpenAI, or any other source into a unified format for analysis and replay.

### Multi-Layer Format Strategy

Different formats serve different layers of the stack:

| Layer | Format | Why |
|-------|--------|-----|
| **Transport** | JSON, WebSocket, etc. | Protocol-dependent |
| **Content schema** | rlog lines | Semantic structure |
| **Local storage** | .rlog files | Human-readable archives |
| **ML export** | ATIF JSON | Training pipelines |

The key insight: rlog lines work as **both** individual event payloads **and** concatenated files. For example, over Nostr (NIP-90), each tool call could become one JSON event with an rlog line as content:

```json
{
  "kind": 7000,
  "content": "t:Read id=call_1 src/auth.rs → [186 lines]",
  "tags": [["status", "processing"], ["e", "<job-id>"]]
}
```

This pattern works across transports—WebSocket streams, HTTP SSE, file tailing, or message queues. The rlog format is transport-agnostic:
- **Real-time**: Each line streams independently
- **Archival**: Aggregate lines into .rlog files for storage/audit
- **Interop**: Convert rlog → ATIF for ML training when needed

### Why Not ATIF Directly?

[ATIF](https://github.com/HarborML/harbor) is excellent for offline ML workflows with `logprobs`, `completion_token_ids`, and reward signals. But it's a document format—you need the complete JSON to parse it. For live coordination where agents stream partial results (NIP-90 `status: "partial"`), you need a line-oriented content schema. rlog provides that, and we can always export to ATIF later.

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
