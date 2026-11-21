# Unreal shipping ZIP previews

From a checked-out GameBuilder tree, run:

```sh
scripts/import-unreal.sh "/path/to/SPLICEBOTS-shipping.zip" \
  --engine "/path/to/UnrealEngine-5.9/src" \
  --out "$HOME/.openagents/splicebots-preview"
```

Use a built Unreal Engine 5.9 source tree that provides UnrealPak for the host and the editor's generated UHT C++ files. Run `scripts/pin.sh sync` to set up GameBuilder's pinned OpenAgents tools. The SPLICEBOTS source checkout is unnecessary. The ZIP can be used as it was shipped, including its Windows path separators. No login, API keys or model calls are part of the job.

The command creates a private directory with two files:

- **`unreal-preview.pack`** is an imported-world pack for the Metaverse Engine's [imported-world walker](https://github.com/OpenAgentsInc/openagents/blob/3109f42ed9/apps/openagents-web/importworld.coax). It contains the surviving scene geometry, materials, lights, spawns and colliders. The pack is a local preview artifact. The native GameBuilder editor and Play integration for imported projects are later milestones.
- **`report.json`** records every ZIP entry with its disposition, the map inventory, the full cooked reader skip list, the converter's counts and per-object notes, and every parameter behind its unmapped-material count. An ignored Pak or loose data file is listed by name. The report also states scope limits for content the cooked input cannot expose. It includes the ZIP and pack SHA-256, the OpenAgents pin, hashes of the supplied UnrealPak and Unreal build metadata, and the UHT schema dependency.

On the original SPLICEBOTS Windows shipping ZIP, the preview contains eight maps, 4,415 packed placements and 2,443 colliders. The report records 174 skipped ZIP files, 996 cooked reader skip records and 3,220 unmapped material parameters. The categories cover different stages and can describe related parts of the same source object.

For a quick read of the report, use `jq '.maps, .conversion_counts' NEW_DIR/report.json`. Use `jq '.cooked_skips[] | select(.reason == "logic")' NEW_DIR/report.json` for the gameplay the cooked reader explicitly marked as skipped.

## Supported profile

This command handles a classic ZIP with one unencrypted, unsigned, unpartitioned version 8 game IoStore pair and an optional global pair beside it. It reads ZIP files up to 1 GiB, with at most 4,096 entries and 256 MiB per selected ZIP member. It validates all central-directory paths before extracting selected members with size and CRC checks. Packed executables and loose files are inventoried as data. For a source ZIP, Pak-only build, split or protected container, missing pair, invalid input or missing toolchain, the command names the blocking stage and exits nonzero.

`--out` needs an existing parent and a new final directory. Staging lives under the same parent. A failed import does not publish a final directory. Keep the original archive and both outputs local for this private game. The command's preview verdict covers pack readback and the reported conversions. It does not certify appearance, behavior or gameplay parity.

## Repeat the check

Run the same command twice into two new directories with the same ZIP, pinned OpenAgents tools and Unreal build, including its generated schemas. Then compare both outputs:

```sh
cmp first/unreal-preview.pack second/unreal-preview.pack
cmp first/report.json second/report.json
```

The report omits scratch paths and elapsed times. Archive entries have stable order and the import receives a stable pack name. `scripts/check.sh` includes the synthetic ZIP admission and report checks, which run without Unreal or the private game ZIP.

See the [port automation audit](unreal-port-automation-audit.md) for the remaining stages.
