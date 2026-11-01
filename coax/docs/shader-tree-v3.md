# Checked shader tree3

The first word selects the operation registry. Tree2 keeps the published
record layout and GPU ids1–14. Tree3 uses that layout and adds:

| Id | Procedure | Fragment effect |
| --- | --- | --- |
| 15 | `gpu_dpdx_fine(f32) -> f32` | Fine X derivative |
| 16 | `gpu_dpdy_fine(f32) -> f32` | Fine Y derivative |
| 17 | `gpu_fwidth_fine(f32) -> f32` | Sum of absolute fine derivatives |
| 18 | `gpu_demote_helper()` | Suppress publication; retain helper execution |

The compiler chooses tree3 only when one of these operations is reachable
from an emitted entry. Both lowerers accept tree2/tree3 and refuse the new
registry under tree2. The SPIR-V lowerer emits opcodes210–212 with capability51
for fine derivatives, and opcode5380 with capability5379 and
`SPV_EXT_demote_to_helper_invocation` for helper conversion. Unsupported device
features must be refused before pipeline creation. WGSL emits the explicit
fine builtins and `discard` for helper conversion.

Old ids12–14 retain generic derivative selection. Old id1 retains terminating
SPIR-V `OpKill`; it is not an alias for helper conversion. The ordinary scalar
CPU library retains zero derivative bodies. Quad execution declares its lane
order and generic derivative selection separately; it cannot infer a unique
generic hardware result from this registry alone.

`gpu_layer_width(Tex2DArray)` and `gpu_layer_height(Tex2DArray)` are typed
aliases of the existing image-query ids10/11. Their scalar bodies return the
declared dimensions; both GPU targets query the bound image's base level.
These aliases do not extend the tree2 operation registry or force tree3.

The SPIR-V command preserves `binding SLOT KIND NAME` report lines. Additional
observations begin with `reflection 2`:

```text
tree-version VERSION
resource SLOT KIND SHAPE STRIDE STAGES NAME
vertex-stride BYTES
vertex-input LOCATION SCALAR WIDTH OFFSET
fragment-targets COUNT
fragment-depth BOOL
derivatives 1 MASK
control 1 MASK
```

Kinds are1 storage,2 image,3 sampler; uniform data lowers to storage. Image
shapes are0 2D,1 array,2 depth; sampler shapes are0 ordinary,1 comparison.
Storage stride is its checked element byte size. Stages are1 vertex,2 fragment,
3 both. Scalar codes are2 f32,3 s32,4 u32. Derivative mask bit0 observes generic
operations and bit1 fine operations; control bit0 observes legacy kill and bit1
helper conversion. The resource names and native slots describe this entry
pair, not a global slot assignment across passes. A logical uniform binding's
declared byte view remains part of Aethyr's separately admitted binding plan.
