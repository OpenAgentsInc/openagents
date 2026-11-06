# Current producer and displayed graphics

The admitted Coax 0.52 producer passes the matched 90-frame handoff. Native
and Wasm use the same pack, facts, frame arguments and strict floating-point
rules. Metal executes both observed handoffs. Vulkan executes the complete
matched Wasm RFI, including the same frozen texture and media inputs.

| Complete comparison | Channels past the unchanged 0.002 rule | Nonfinite / masked |
| --- | ---: | ---: |
| Actual native FP16 graph vs matched Wasm graph through Metal | 0 / 230,400 | 0 / 0 |
| Actual native displayed BGRA8 vs matched Wasm displayed RGBA8 through Vulkan | 1,915 / 230,400 | 0 / 0 |

The second row passes the inherited G5 live allowance of 2,000. The Metal
offscreen driver explicitly requests the same quantized v1 presentation
descriptor that the ordinary drawable path selects. This numerical replay
does not by itself qualify a window, device lifecycle or desktop default.
The ordinary Linux window and current browser pack/recovery workloads have
separate physical receipts.

The retained [FP16-to-UNORM attempt](oa-12869-native-wasm-vulkan.json)
reports 3,240 outliers and fails. It compares the internal graph to displayed
UNORM output. The corrected
[display-to-display measurement](oa-12869-native-wasm-vulkan-presented.json)
reads the actual quantized native display; no tolerance or mask changes.
The [Metal handoff](oa-12869-native-wasm-metal.json) has zero outliers.
The [Vulkan measurement](measurement.json) retains unchanged local, remote
and SDK fences, owned cleanup, 137 actual draws and zero reported validation
errors or warnings. The separate ordinary window explicitly requires
validation observed enabled.

## Capture ownership and provenance

`capture-render-frame.coax` now captures optional media only when the face
exports its measured allocation. The new span authority requires the entire
allocation to fit actual Wasm memory before copying the logical bytes.
It kills [21/21 root mutants](media-mutants.log), without equivalents.
Older faces without measured media keep their prior output and make no media
observation. The actual [one-frame fixture](optional-media-absent.json)
passes with no media output.

The initial comparer rejects the existing Coax observer's `source_inputs`
provenance even though all 54 raw artifacts match. The
[red regression](provenance-red-test.log) catches that mismatch. The fix
allows that known provenance field alongside `tool`; all 14 product fields,
unknown-left-field refusal and every raw byte stay checked. The authority
kills [90/90 root mutants](provenance-mutants.log). The
[final join](observer-provenance-green.json) proves all 54 artifacts and
12,668,276 bytes match the earlier GPU inputs. Existing passing GPU outputs
are carried by that exact byte join rather than rendered again.

The [final capture](final-wasm90-capture.json) records the committed observer
closure. It retains actual positive uploads separately from padded physical
buffers. The handoff view selects those observed upload files; it does not
overwrite the original captures. The measured allocation capacities come
from the capture's actual export observations. Native and Wasm media command
bytes also match exactly (SHA-256
`21f29b6a3d769dd13066d282433fe28d302c98f72e04f3696589cc90cf963f16`).

Remote SDK compilation runs at nice 15. Compiler build admission uses the
original, unmodified provenance receipts in the parent directory. The
separate `observed-source-digest.txt` is a preparation observation, not a
replacement build receipt.
