# 271. Run Inkling 2 Mini in OpenAgents Desktop

Part XXVII culminates in making Inkling 2 Mini a first-class, selectable `$0.00`
local model inside OpenAgents Desktop (`apps/openagents-desktop/` across macOS
Metal 4, Linux Vulkan/CUDA, and Windows Win32/CUDA `#14046`), backed by the
native Metal 4 and CUDA runtimes ([chapter 269](ch269-inkling-2-mini-on-metal.md))
and the `/v1/responses` + strict JSON-schema server
([chapter 270](ch270-tool-calls-and-responses-for-inkling-2-mini.md)).

- `#authority` hardware/disk/SHA-256 readiness gating:
  [`psionic/lib/inkling-desktop.coax`](../lib/inkling-desktop.coax)
- Runnable verification example:
  [`psionic/examples/inkling-desktop.coax`](../examples/inkling-desktop.coax)
- OpenAgents Desktop integration:
  - Composer model picker, multi-variant GGUF/Maple catalog (`#14028`, `feff1652bc`), Linux/Windows variant picker (`3c3a222ce2`), and `--model inkling-2-mini` launch support:
    [`apps/openagents-desktop/page/models.coax`](../../apps/openagents-desktop/page/models.coax),
    [`apps/openagents-desktop/page/layout/conversation.coax`](../../apps/openagents-desktop/page/layout/conversation.coax),
    [`apps/openagents-desktop/host-core.coax`](../../apps/openagents-desktop/host-core.coax),
    [`apps/openagents-desktop/Window.swift`](../../apps/openagents-desktop/Window.swift)
  - Verified Local Models downloader & SHA-256 store (`~/.openagents/models/inkling-2-mini/`):
    [`apps/openagents-desktop/LocalModels.swift`](../../apps/openagents-desktop/LocalModels.swift)
  - Resident 32K-context Psionic Metal/CUDA server lifecycle, cloud-first `$0.00` local tier onboarding (`#14072`, `#14087`), and unified `openagents/core` (`oa-worker.coax`) + `Inky` (`coax/lib/inky/`) routing:
    [`apps/openagents-desktop/Window+Dialogs.swift`](../../apps/openagents-desktop/Window+Dialogs.swift),
    [`apps/openagents-desktop/Window+Runs.swift`](../../apps/openagents-desktop/Window+Runs.swift),
    [`apps/openagents-desktop/page/usage.coax`](../../apps/openagents-desktop/page/usage.coax),
    [`coder/tui/oa-worker.coax`](../../openagents/runtime/oa-worker.coax),
    [`apps/openagents-desktop/AppDelegate.swift`](../../apps/openagents-desktop/AppDelegate.swift)

```shell
$ coax run psionic/examples/inkling-desktop.coax
Inkling 2 Mini (Local) (Installed (17.63 GB · SHA-256 verified)): selectable=true ctx=32768
16GB Mac: Requires 32 GB RAM (selectable=false)
64GB Mac verifying 64: Verifying SHA-256... (selectable=false)
64GB Mac paused 42: Paused (resume available) (selectable=false)
64GB Mac low disk: Insufficient disk space (need 17.63 GB) (selectable=false)
```

## Hardware, disk, and SHA-256 readiness gating

Before `Inkling 2 Mini (Local)` can become selectable in the composer model
picker, Desktop enforces three non-negotiable gates:

1. **Unified memory floor (`>= 32 GB`)**: Macs with `< 32768 MiB` unified
   memory are refused as `unsupported_hardware` even if weight files exist on
   disk.
2. **Free disk headroom (`>= 16812 + 512 MiB`)**: prevents starting a 17.63 GB
   download onto a volume that cannot hold the checkpoint plus atomic promotion
   headroom.
3. **Streaming SHA-256 verification**: `LocalModelStore` verifies every
   downloaded or pre-existing `Inkling-2-Mini-IQ4_NL.gguf` (`17,627,940,768`
   bytes) in 4 MiB chunks via `CryptoKit.SHA256` against the pinned Hugging Face
   LFS digest (`cbdf3fa9dc50a4124977980f37da4f1fa764b0cfe800d275cd43d26ce2a81af4`)
   and writes a tamper-evident `manifest.json` keyed on exact size and file
   `mtime`. Corrupt or truncated files are discarded and never marked selectable.

<!-- source: psionic/lib/inkling-desktop.coax readiness -->
```jai
INKLING_MODEL_ID :: "inkling-2-mini";
INKLING_MODEL_LABEL :: "Inkling 2 Mini (Local)";
INKLING_MIN_RAM_MB :: 32768;
INKLING_FOOTPRINT_MB :: 16812;       // 17,627,940,768 bytes (17.63 GB IQ4_NL GGUF)
INKLING_DISK_HEADROOM_MB :: 512;
INKLING_CTX_TOKENS :: 32768;         // aligned with card.coax and LocalModelServer --ctx 32768

InklingLocalState :: enum u8 {
  not_installed;
  downloading;
  paused;
  verifying;
  installed;
  unsupported_hardware;
  insufficient_disk;
}

InklingDesktopStatus :: struct {
  state: InklingLocalState;
  selectable: bool;
  ram_mb: int;
  free_disk_mb: int;
  footprint_mb: int;
  ctx_tokens: int;
  download_pct: int;
  sha_verified: bool;
  status_code: int;
}

inkling_clamp_pct :: (pct: int) -> int #effects(all) {
  if pct < 0 { return 0; } // mutant: equivalent — pct == 0 returns 0 whether clamped or returned directly
  if pct > 100 { return 100; } // mutant: equivalent — pct == 100 returns 100 whether clamped or returned directly
  return pct;
}

// Evaluate whether Inkling 2 Mini can be selected or downloaded in OpenAgents Desktop.
// Requires >= 32 GB unified memory and a SHA-256 verified local checkpoint before selection.
inkling_desktop_readiness :: (
  ram_mb: int, free_disk_mb: int, installed_verified: bool,
  downloading: bool, paused: bool, verifying: bool, download_pct: int
) -> InklingDesktopStatus #export_c #effects(all) {
  if ram_mb < INKLING_MIN_RAM_MB {
    return InklingDesktopStatus.{
      state = InklingLocalState.unsupported_hardware, selectable = false,
      ram_mb = ram_mb, free_disk_mb = free_disk_mb, footprint_mb = INKLING_FOOTPRINT_MB,
      ctx_tokens = INKLING_CTX_TOKENS, download_pct = 0, sha_verified = false, status_code = 1
    };
  }
  if verifying {
    return InklingDesktopStatus.{
      state = InklingLocalState.verifying, selectable = false,
      ram_mb = ram_mb, free_disk_mb = free_disk_mb, footprint_mb = INKLING_FOOTPRINT_MB,
      ctx_tokens = INKLING_CTX_TOKENS, download_pct = inkling_clamp_pct(download_pct), sha_verified = false, status_code = 4
    };
  }
  if downloading {
    return InklingDesktopStatus.{
      state = InklingLocalState.downloading, selectable = false,
      ram_mb = ram_mb, free_disk_mb = free_disk_mb, footprint_mb = INKLING_FOOTPRINT_MB,
      ctx_tokens = INKLING_CTX_TOKENS, download_pct = inkling_clamp_pct(download_pct), sha_verified = false, status_code = 2
    };
  }
  if installed_verified {
    return InklingDesktopStatus.{
      state = InklingLocalState.installed, selectable = true,
      ram_mb = ram_mb, free_disk_mb = free_disk_mb, footprint_mb = INKLING_FOOTPRINT_MB,
      ctx_tokens = INKLING_CTX_TOKENS, download_pct = 100, sha_verified = true, status_code = 0
    };
  }
  if paused {
    return InklingDesktopStatus.{
      state = InklingLocalState.paused, selectable = false,
      ram_mb = ram_mb, free_disk_mb = free_disk_mb, footprint_mb = INKLING_FOOTPRINT_MB,
      ctx_tokens = INKLING_CTX_TOKENS, download_pct = inkling_clamp_pct(download_pct), sha_verified = false, status_code = 5
    };
  }
  if free_disk_mb < INKLING_FOOTPRINT_MB + INKLING_DISK_HEADROOM_MB {
    return InklingDesktopStatus.{
      state = InklingLocalState.insufficient_disk, selectable = false,
      ram_mb = ram_mb, free_disk_mb = free_disk_mb, footprint_mb = INKLING_FOOTPRINT_MB,
      ctx_tokens = INKLING_CTX_TOKENS, download_pct = 0, sha_verified = false, status_code = 6
    };
  }
  return InklingDesktopStatus.{
    state = InklingLocalState.not_installed, selectable = false,
    ram_mb = ram_mb, free_disk_mb = free_disk_mb, footprint_mb = INKLING_FOOTPRINT_MB,
    ctx_tokens = INKLING_CTX_TOKENS, download_pct = 0, sha_verified = false, status_code = 3
  };
}

inkling_desktop_status :: (ram_mb: int, installed: bool, downloading: bool, download_pct: int) -> InklingDesktopStatus #export_c #effects(all) {
  return inkling_desktop_readiness(ram_mb, INKLING_FOOTPRINT_MB + INKLING_DISK_HEADROOM_MB, installed, downloading, false, false, download_pct);
}

// Build the concise status summary shown in OpenAgents Desktop Settings > Models and composer picker.
inkling_desktop_summary :: (st: InklingDesktopStatus) -> string #effects(all) {
  if st.state == InklingLocalState.unsupported_hardware { return "Requires 32 GB RAM"; }
  if st.state == InklingLocalState.installed { return "Installed (17.63 GB · SHA-256 verified)"; }
  if st.state == InklingLocalState.verifying { return "Verifying SHA-256..."; }
  if st.state == InklingLocalState.downloading { return "Downloading..."; }
  if st.state == InklingLocalState.paused { return "Paused (resume available)"; }
  if st.state == InklingLocalState.insufficient_disk { return "Insufficient disk space (need 17.63 GB)"; }
  return "Not downloaded (17.63 GB)";
}
```

## Composer model picker, `--model inkling-2-mini`, and resident runner

1. **Composer model picker (`page/models.coax`, `page/layout/conversation.coax`)**:
   when Inkling 2 Mini is verified on a `>= 32 GB` Mac, `Inkling 2 Mini (Local)`
   appears as a selectable model with backend label `Psionic Metal`. The picker
   footer row (`coder.model.local`) continuously displays live availability,
   download progress/speed/ETA, or hardware status and opens the Local Models
   manager dialog on click.
2. **CLI launch flag (`--model inkling-2-mini`)**: selects `Inkling 2 Mini (Local)`
   immediately when installed, opens the Local Models manager when weights are
   not yet downloaded, or presents a clear 32 GB RAM notice on smaller Macs.
3. **Resident Psionic Metal 4 / CUDA server (`LocalModelServer` & `host-main.c`)**:
   launches the bundled `psionic-inkling-serve` (Metal 4 on macOS, CUDA on
   Linux/Windows) on `http://127.0.0.1:18794` with `--ctx 32768` (aligned with
   `coax/lib/gateway/card.coax`), logs stderr to
   `~/.openagents/logs/psionic-inkling-serve.log`, detects port conflicts and
   early exits immediately, routes the unified `openagents/core` worker
   (`coder/tui/oa-worker.coax`, `#14050`–`#14053`) and `Inky`
   (`coax/lib/inky/`; Microcoder `mcworker` is deprecated `#13983`) via
   `PSIONIC_BASE_URL` + `PSIONIC_MODEL=inkling-2-mini`, enforces `$0.00` local
   compute accounting in `page/usage.coax` beside OpenAgents Cloud onboarding
   (`#14072`, `#14087`), and stops cleanly on app termination
   (`AppDelegate.applicationWillTerminate`) or model deletion.

<!-- source: psionic/examples/inkling-desktop.coax example -->
```jai
main :: () {
  ready := inkling_desktop_readiness(32768, 40000, true, false, false, false, 100);
  low := inkling_desktop_readiness(16384, 40000, true, false, false, false, 100);
  verifying := inkling_desktop_readiness(65536, 40000, false, false, false, true, 64);
  paused := inkling_desktop_readiness(65536, 40000, false, false, true, false, 42);
  disk_low := inkling_desktop_readiness(65536, 12000, false, false, false, false, 0);
  print("% (%): selectable=% ctx=%\n", INKLING_MODEL_LABEL, inkling_desktop_summary(ready), ready.selectable, ready.ctx_tokens);
  print("16GB Mac: % (selectable=%)\n", inkling_desktop_summary(low), low.selectable);
  print("64GB Mac verifying %: % (selectable=%)\n", verifying.download_pct, inkling_desktop_summary(verifying), verifying.selectable);
  print("64GB Mac paused %: % (selectable=%)\n", paused.download_pct, inkling_desktop_summary(paused), paused.selectable);
  print("64GB Mac low disk: % (selectable=%)\n", inkling_desktop_summary(disk_low), disk_low.selectable);
}
```

## Live end-to-end verification

```text
1. #authority checks & mutation sweep:
   coax/bin/coax test psionic/lib/inkling-desktop.coax -> 21 checks, 21 passed, 0 failed
   coax/bin/coax mutate psionic/lib/inkling-desktop.coax --changed origin/main
   -> 72 mutants, 69 killed, 0 survived, 3 marked equivalent
2. Live bundled mcworker + psionic-inkling-serve (--ctx 32768) on Apple M5 Max:
   HEALTH: {"status":"ok","engine":"psionic-metal"}
   Microcoder tool-use turn reading temp workspace secret.txt:
   23 streamed events, tool executed (SAW_TOOL=True),
   {"kind":"ended","ending":"finished","text":"OPENAGENTS_INKLING_METAL_32K_OK"}
3. OpenAgents Desktop build, checks, and Coax page suite:
   apps/openagents-desktop/check.sh -> openagents-desktop check: ok (incl. 11/11 local-models-check)
   coax/bin/coax test apps/openagents-desktop/page.coax --library -> 843 checks, 843 passed, 0 failed
4. Book check:
   python3 psionic/tests/book.py --book psionic -> pass
```
