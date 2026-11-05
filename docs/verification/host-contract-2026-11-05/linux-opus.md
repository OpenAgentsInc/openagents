# Linux authored encoded-audio component

The actual Linux adapter passes all 153 authored candidates against the
unchanged frozen browser PCM: 12,657,718 samples, zero beyond 2^-20 and zero
nonfinite values, without alignment, shifts or masks. All 32 required APC1
companions are byte-identical. The other 121 use the installed libopusfile
0.12/libopus 1.6.1 SDK. Maximum observed absolute error is
2.682209014892578e-7 (`frost-cast-e1`). Original Ogg sources and the existing
catalogue/companion selection remain unchanged.

`verse/native/audio-opus.c` executes the shared source/name/catalogue,
preflight, post-SDK shape, encoded-byte and PCM reservation policies before
copying or publishing their corresponding planes. The new Coax read
controller requires exact trimmed output followed by an independent EOF
probe; negative, premature, extra or oversized results refuse the complete
stream. Finite-word admission precedes retained PCM copying. The source,
companion, complete coded plane, probe and retained output own separate
shared tickets. SDK-private decoder workspace is not an observed PCM plane.
The installed SDK's [float-read contract](https://opus-codec.org/docs/opusfile_api-0.12/group__stream__decoding.html)
supplies interleaved native-endian F32 at 48 kHz, with word capacities and
frame return counts; the Coax controller explicitly converts that boundary.

Actual normal and ASan/UBSan executions pass the same source/shape/cleanup
and seven-case refusal receipts. Missing or corrupt required companions,
corrupt original sources and exhausted 32 MiB shared memory reach no SDK
decode and publish no PCM. A later valid required companion, listed SDK
asset and unlisted safe SDK name load independently. Every row releases to
zero words. Instrumentation includes generated Coax C, adapter, allocation
wrapper and fixture effects; installed codec libraries are not instrumented.
The shared read controller kills 59/59 root mutants and finite publication
kills 5/5; existing source, memory, companion, catalogue, comparator and
receipt authorities retain their earlier unchanged passing gates.

The exact generated component and combined observation tool use the
original admitted 598 compiler, genuine c224 protected compiler checkout,
and separately overlaid product inputs. Its original receipt verifies before
and after overlay and after emission. Actual C source tables join all 30
component and 42 observation-tool inputs with zero differences. This writes
no compiler receipt and claims no current compiler release. A combined tool
graph avoids duplicate exported digest kernels between independently emitted
product and checker units; the first duplicate-symbol link failure is retained.
The initial old-overlay emission is retained separately and is not admitted
as the current component.

Receipts and exact build inputs are in [linux-opus/](linux-opus/). Complete
emission, raw samples, logs and SDK/reference identities remain under
`~/.openagents/aethyrforge/host-contract-12869/linux-opus/` on the Mac and
coderos-4080. The work uses the new isolated remote worktree; the earlier
remote worktree's unrelated retained edits remain untouched. No GPU,
PipeWire output device, visible browser or audible sound runs for this gate.

Reproduction from the recorded admitted emission:

```sh
build-linux-audio-opus ROOT EMISSION NEW_OUT OPUSFILE.so [--sanitize]
NEW_OUT/check-linux-audio-opus ROOT PICKED_BROWSER EXTRA_BROWSER RESULT_DIR
```

This closes encoded decoding and its allocation/refusal component only.
Ordinary Linux lazy source registration, complete default demand, rendered
default cues, realm/device integration and the complete graphics/host/default
admission remain separate required exits. The desktop default stays web.
