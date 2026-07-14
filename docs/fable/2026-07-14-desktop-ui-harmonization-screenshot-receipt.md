# OpenAgents Desktop UI harmonization screenshot receipt

Date: 2026-07-14  
Issue: `OpenAgentsInc/openagents#8811`  
Baseline: `7de092387e96dd4cefdcea2a8b81b48787c704d0`  
Capture: built Electron smoke host, 2400×1600 PNGs, dark `khalaTheme`

The smoke host captured the same deterministic fixture before and after the
final CSS/catalog consolidation. Every behavioral assertion passed. Manual
inspection caught and rejected an intermediate build whose dock labels,
history wrapping, and centered command rows regressed; the final capture below
restores those layouts. The remaining deliberate visual changes are catalog
adoption: command chords use the typed outline `Badge`, dimension-token panes
use the shared lattice, and component appearance comes from render-dom rather
than app-local CSS recipes.

| Surface | Final SHA-256 | Changed pixels vs baseline | Adjudication |
| --- | --- | ---: | --- |
| shell | `b889cb0cc631b606c1024b8f6dbbd8fb6cc524b18187c27db34e75928b222f1b` | 2.56% | pass |
| command palette | `581925328017aabcb99f16b9b963a0cdd00be6a5e3ce66010327362201c34a86` | 15.65% | pass — typed Badge keycaps |
| Codex history detail | `b07b4be4b2ece095a3fb914a112f4b07de65276955f61044e7eefbb718b9e9aa` | 4.92% | pass |
| current Codex settings | `fb11c34504db516691d7eb679d73ca8d41d1dd50252e8ec7bb34f4c6d53af482` | 0.20% | pass |
| diagnostics | `334f8c6f19524ac4eb62c74180e23106ed10f7cc5dc47a91015d95090c12f761` | 0.20% | pass |
| files workspace | `fc1d52c2e8cf25da4e730dd09073ea95ad462d45f320f3d4829dde7403220969` | 11.94% | pass — shared pane lattice |
| empty chat | `312ffe92edd81fb16ade08b31ad27f81f68e9224d1919007c2de98b42076ac1f` | 2.70% | pass |
| message inspector | `4b4f97aa59d5337d3e0749101951bbbea126f44c40e600ffd635c03de1912268` | final-only | pass |
| coding catalog | `4573c02140ba033c22b0b51204141bfce4e42ba2920dcbc47c06446a369b31bf` | final-only | pass |
| streamed Codex turn | `244d7240ce440aa92403cc356d2d8b0261106457868709ef0b43ba2fd9efc0c6` | 10.61% | pass |
| Git review | `89f222a15889f2ce2241c41f451bafda8bf85870474faef2a949513418a43def` | final-only | pass |
| filtered sessions | `271025e0e814926db7d390df58024d2aa41ad91456bbe17ca4abbac97e637a01` | 2.32% | pass |

Capture command:

```sh
OPENAGENTS_DESKTOP_SMOKE_SHOTS=<receipt-dir> pnpm --filter @openagentsinc/openagents-desktop run smoke
```

The PNGs are test artifacts rather than source assets; hashes make the receipt
tamper-evident without adding roughly 13 MiB of generated binaries to Git.
