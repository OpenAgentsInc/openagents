# Linux native distribution reconciliation — #8921

- Date: 2026-07-19
- Issue: [#8921](https://github.com/OpenAgentsInc/openagents/issues/8921)
- Source tested: `ead5ba8b5e5001f5f815b6f7864f325f794e5614` (`0.1.0-rc.23`)
- Follow-up source: `73bf298d5e3e6345fced03ac12346439096b22e5` (`0.1.0-rc.24`)
- Hosts: owned Ubuntu 24.04 x64/arm64 builders and ephemeral Rocky Linux 9.8 x64/arm64 acceptance hosts
- Status: substantial native evidence landed; issue is not yet closeable

## Implemented

The Desktop main process now selects an AppImage-specific Linux applier. It
verifies AppImage/ELF identity and architecture, stages full images in private
user-owned storage, atomically swaps the selected symlink, retains exactly one
previous image, and drives durable first-launch healthy/rollback transactions.
The reusable native harness is
`apps/openagents-desktop/scripts/linux-appimage-native-acceptance.ts`.

The Linux makers now freeze the application-ID desktop filename, executable
symlink, icon identity, `StartupWMClass`, and protocol MIME registration for
AppImage, DEB, and RPM. The bounded patch to
`electron-installer-common@0.10.4` separates package identity from desktop and
binary identity; package IDs remain `openagents-desktop[-rc]`.

## RC23 artifact evidence

| Target | Format | SHA-256 |
| --- | --- | --- |
| linux-x64 | AppImage | `d15b456c8f3b00bf226150c89863d6039dc6ad16f1e4f24ae4b9224dc6ddb3a1` |
| linux-x64 | DEB | `ff2f84f596f4e8d6158e68799695e22e51e7dec206ce58fde7f6f7807dcea015` |
| linux-x64 | RPM | `c755be220cb9262a3420a359b84245db013581d63881b6e61ae3f22c434eb60f` |
| linux-arm64 | AppImage | `49983a3607e725bb5af11d9a4756e59fb0fc19831fd40191b9b5e95decd95590` |
| linux-arm64 | DEB | `1bda479d4ab93204f5296e86f31eeafa5ecf12092cec05a2fad2179e79000ebb` |
| linux-arm64 | RPM | `6057456bbf03bf6358ba930a48655b15070c027c8e6ecd3380d67ad2f8b6f914` |

Both AppImages had the correct native ELF machine. DEB metadata reported
`openagents-desktop-rc`, `0.1.0~rc.23`, `amd64`/`arm64`, and OpenAgents, Inc.
RPM metadata reported `openagents-desktop-rc`, `0.1.0.rc.23-1`,
`x86_64`/`aarch64`, and the expected product metadata.

## Native lifecycle results

- AppImage x64 and arm64: exact public RC21 digest -> RC23 install passed;
  selected bytes and `0755` mode passed; expired first-launch watchdog restored
  exact RC21 bytes; receipt-bound clean shutdown retained RC23 and removed the
  previous slot. Both AppImages mounted the real Desktop shell under Xvfb.
- DEB x64 and arm64 on Ubuntu: RC21 install -> RC23 upgrade, reinstall,
  uninstall, shell mount, and user-data preservation passed.
- RPM x64 and arm64 on Rocky 9.8: RC21 install -> RC23 upgrade, reinstall,
  uninstall, shell mount, and user-data preservation passed.
- All four package-manager runs exposed the same RC23 desktop integration
  defects: package-name desktop filenames and missing protocol MIME entries.
  RC24 fixes those defects in source and advances the version so RC23 bytes are
  never reused.

The packaged Electron smoke consistently mounted the shell, composer, runtime
gateway, workspace files, lifecycle, commands, and recent history on all four
Linux hosts. Its deep trace step then failed `history_modifier_scroll_reset`
under Xvfb; this is recorded as a failure, not converted to a pass.

## Honest residuals

1. RC24 still needs native six-cell artifact inspection to bind the corrected
   desktop integration to produced bytes.
2. The current ProductSpec treats Codex as an externally maintained native
   harness, while #8921 still says “bundled agent runtimes.” The package does
   include the target-specific Claude provider runtime and owned audio helper,
   but not a bundled Codex binary. The issue wording and current ownership
   contract must be reconciled before that criterion can pass.
3. The Xvfb deep-scroll smoke failure needs a Linux-specific diagnosis or a
   representative interactive-session pass.
4. The six Linux artifacts are not yet members of one signed ReleaseSet v2;
   that requires the open matrix/promotion dependencies #8917, #8925, and
   #8926. No stable feed or `/download` support claim was promoted here.

Accordingly #8921 remains open. The evidence narrows it to corrected RC24
package proof, Linux headed acceptance, ownership reconciliation, and signed
matrix convergence; AppImage update/rollback itself is now implemented and
natively proven on both architectures.
