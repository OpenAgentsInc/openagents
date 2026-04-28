# Issue 4468 Native Windows Support Progress

Date: 2026-04-27

Issue: `OpenAgentsInc/openagents#4468`

## Summary

The code path for native Windows packaging is now implemented on `main`:

- `packages/pylon-bootstrap` now resolves `win32` to `windows`
- Windows release assets use `pylon-v<version>-windows-x86_64.zip`
- cached install paths use `pylon.exe` and `pylon-tui.exe`
- the bootstrap source-build fallback now looks for Windows `.exe` artifacts
- Windows prebuilt extraction uses PowerShell `Expand-Archive`
- `scripts/release/pylon-binary-release.sh` now supports a native Windows host,
  emits `.zip` assets, and packages `.exe` binaries into the archive

## Local Verification

Passed from `openagents` repo root:

```bash
bun test packages/pylon-bootstrap/test/bootstrap.test.js packages/pylon-bootstrap/test/cli.test.js
bash -n scripts/release/pylon-binary-release.sh
```

The bootstrap tests now cover:

- `resolvePlatformTarget("win32", "x64")`
- Windows asset naming
- Windows prebuilt install paths with `pylon.exe` / `pylon-tui.exe`
- Windows source-build fallback using `.exe` artifacts

## Remaining Closure Blocker

This Mac does not provide a native Windows host, and the workspace does not
currently expose one over Tailnet or an equivalent remote operator path.

Because of that, the final issue acceptance is still pending:

- real `pylon-v<version>-windows-x86_64.zip` publication
- native Windows smoke for `pylon.exe`, `pylon-tui.exe`, and the npm bootstrap

## Conclusion

Keep `#4468` open until a native Windows x86_64 host runs the retained smoke
proof and the real release asset is published.
