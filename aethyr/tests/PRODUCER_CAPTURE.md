# Producer capture port

`capture-render-frame.coax` owns producer options, initialization order, event
delivery, frame scheduling, resource retention and capture publication. Its
JavaScript adapter performs WebAssembly and browser SDK effects only. The old
TypeScript observer is retained as a hash-pinned reference in the
[port evidence](../../docs/verification/aethyr-producer-capture-port-2026-11-02.json).

```sh
COAX=/path/to/qualified/coax coax/web/tool.sh aethyr/tests/capture-render-frame.coax PRODUCER.wasm --dump NEW_DIRECTORY --frames 120 --every 30 --size 1280x720
```

The existing `--scale`, `--clock`, `--page`, `--pack`, `--facts`, `--fact`,
`--extra`, `--hash`, `--keys`, `--pointer`, `--index`, `--vertices`, `--mesh`
and `--golden` options are preserved. Clocks retain signed 64-bit integers,
including leading zeros. Facts use the legacy UTF-8 replacement semantics,
preserve BOM and NUL, trim trailing LF and append `--fact` only with `--facts`.
Count-only execution skips artifact getters and removes its owned temporary
directory. A missing required export stops before subsequent producer calls.

The capture remains `captured-unadmitted`: it records producer bytes, not a
GPU or renderer qualification. Files and Wasm memory are bounded by 64 MiB;
frames are bounded by 600 and each viewport dimension by 8192. Source, tool,
browser and transport identities are fenced before publication. An output
directory must be new.

The port record retains exact Grid artifact parity and 23 distinct tiny
producer cases across the declared source epochs. It includes count-only
output, constructor memory growth, getter order, shrinking and zero stores,
particles, invalid spans and CLI inputs. The final two cases cover malformed
UTF-8 and a leading-zero minimum clock using the same retained fixture Wasm.
No passing producer or GPU gate was repeated for these focused extensions.

To rerun the fixture qualification, extract its reference archive first and
pass `--reference-source PATH/reference/capture-render-frame.ts` to
`check-producer-capture-fixtures.coax`, together with the explicit observer,
observer hash, Bun, parity-checker and compiler options. The live observer is
Coax; Bun is used only to run the archived independent comparison.

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

The retained [session qualification](../../docs/verification/data/aethyr-ch11-2026-11-02/producer-capture-session/qualification.json)
qualifies the allocation prerequisite. The port record adds complete producer
capture parity. Both name their actual older compiler; neither asserts a
matching current-main compiler or final chapter-wide qualification.
