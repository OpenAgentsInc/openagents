# SPLICEBOTS shipping build port

Use the [project port](splicebots-project-port.md) to build from a full
Unreal checkout.

From GameBuilder, with the original 15 November 2026 Windows shipping ZIP:

```sh
scripts/port-splicebots.sh \
  "$HOME/Downloads/OLD-SPLICEBOTS-Win64-Shipping-20261115-152753.zip" \
  --assets "$HOME/.openagents/verse/private-build/web" \
  --tuning "$HOME/games/SPLICEBOTS-jam/Config/DefaultSpliceTuning.ini" \
  --out "$HOME/.openagents/splicebots-playable"
```

Run the generated `play.sh` and open the loopback address it prints. The
optional first argument is the port (default 8828). The player needs a browser
with WebGPU. Python 3 supplies its local static file server. The build needs
the compiler at the GameBuilder pin (`scripts/pin.sh sync`) and the [pinned
engine's Wasm toolchain](https://github.com/OpenAgentsInc/openagents/blob/cf1e4f5cb35f5756b59b980e93a065e9b6f46d95/coax/web/toolchain.sh).
It assembles a standalone local browser game. The native app and editor are
separate targets.

## Inputs and output

The [v1 adapter](../tools/splicebots-adapter-v1.coax) accepts exactly the
original archive and freezes three source supplements by SHA-256:

| Input | Use |
| --- | --- |
| Shipping ZIP | The command extracts `SPLICEBOTS/Data/` directly. The shipping tables are kept over any source project copies. These supply pickups, items, cards, story strings and audio, and are compared to the data pack before that pack is used to build the runtime tables and sounds. |
| `splice-bots-jam.pack` | The source Editor export at `70c5bad`, built with the editor plan and the jam's toon mapping. It supplies the imported world, collision, actors and the pickup sound. |
| `splice-bots-jam-chars.pack` | The retargeted Director and raiders from the source rigs. |
| `DefaultSpliceTuning.ini` | The matching source tuning file. The loose shipping Data does not contain that file. |

The Coax rules and their adapter for the source content come from the
OpenAgents pin. Commands for rebuilding the private world and rigs are in
the [original port notes](splice-bots-port.md) and the [upstream jam port plan](https://github.com/OpenAgentsInc/openagents/blob/cf1e4f5cb35f5756b59b980e93a065e9b6f46d95/docs/unreal/2026-11-16-splice-bots-jam-port-plan.md).
Missing or different supplements fail the build. The Windows executable and
cooked container are not needed at runtime. The command uses zero model calls.
Keep the source packs, generated player and its evidence under a local private
directory. They contain the game's private art.

The output includes the assets, Wasm and shaders used by the player, plus
`port.json` with the input identities, output digests and the two verification
logs. Repeating the command into a fresh output directory on the same pinned
toolchain should reproduce the same file bytes. Compare complete builds with
`diff -qr FIRST SECOND`. The static server binds to loopback.

## Verification scope

The exact Windows test build, route, and evidence for each checkpoint are
in the [campaign comparison report](splicebots-windows-verification.md).

- **Exact shipping archive.** The command checks its digest and ZIP members.
  The engine's `jam-data-check` compares converted layout rows to the ZIP's
  CSV, the converted tables line for line, and the sound and icon inventories.
  This build has 469 stated layout rows that match and 30 with unstated fields.
  It carries 29 tables, including the source tuning file, nine shipped WAVs
  and 38 icons in the intermediate data pack. The browser uses the supported
  subset in its runtime table and sound packs.
- **Coax campaign.** The command runs the pinned `jam-run-check` against the
  packaged world, cast and tables. It covers the route to the ending, with
  separate keyed checks for the farm, harvesting, Digger, the drill route,
  jetpack, boss and night exit. The full route includes shortcuts between
  parts separately exercised by the replay. The browser test is an independent
  check of the actual WebGPU player and input path.
- **Shipping executable.** Attempts to run the original Windows executable
  under CrossOver on the build Mac reached Unreal's crash reporting setup
  before a playable window, with both Direct3D 11 and 12. No side-by-side
  executable replay was recorded. The verification establishes fidelity
  for the shipped loose data and playability of the Coax build.

To run the browser check while the generated player is serving:

```sh
vendor/openagents/coax/web/tool.sh tools/splicebots-smoke.coax \
  http://127.0.0.1:8828/ /tmp/splicebots-farm.png
```

The Coax port opens through its own character selection and uses its own
renderer and interfaces. It connects route maps in a shared world. Some
materials and effects have engine equivalents, and the runtime declares
different bindings where the original Unreal flow has no Coax equivalent.
These are adapter choices, outside byte or frame parity with the Windows
game. See the [Unreal audit](unreal-port-automation-audit.md) for what a general
game port would need.
