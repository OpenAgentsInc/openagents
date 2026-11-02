# Producer capture port

The reusable Coax session and narrow WebAssembly SDK adapter are qualified as
the foundation for porting `capture-render-frame.ts`. The existing driver
remains until its options, frame sequence and output artifacts have parity.

Run the bounded session check from the repository root with an explicitly
selected compiler:

```sh
COAX=/path/to/qualified/coax coax/web/tool.sh aethyr/tests/check-producer-capture-session.coax PRODUCER.wasm NEW_OUTPUT_DIRECTORY
```

It checks source-buffer identity, real allocation, binary transport, optional
export observation and actual invalid-span refusal. It initializes defaults
but advances no producer frame. It writes the origin journal, raw SDK refusal
responses, `sdk-refusal-controls.json` and `qualification.json`. Reusing an
output directory is refused even when it has no prior configuration or
qualification. This preserves inputs that happen to use an observer output
filename inside an existing directory.

The pure admission rules are in `producer-capture-policy.coax`; the check-first
cases are in `producer-capture-checks.coax`. The final policy kills 211/211
root mutants. `producer-capture-session.coax` schedules individual operations;
`producer-capture-effects.js` performs only their browser SDK effects.

The retained [qualification](../../docs/verification/data/aethyr-ch11-2026-11-02/producer-capture-session/qualification.json)
distinguishes this prerequisite from full producer capture and final current
compiler/source qualification.
