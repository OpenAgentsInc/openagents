# iOS Black Screen on TestFlight (Release / Archive)

**Symptom:** App works in development (Debug build, simulator or device). After archiving and installing via TestFlight, the app shows only a black screen.

**Status:** Unresolved. This document records the architecture, what was tried, and recommended next steps.

---

## 1. Architecture: How the first screen is drawn

### 1.1 UI flow

1. **`AutopilotApp`** → **`ContentView`** → **`WgpuiBackgroundView()`**
2. **`WgpuiBackgroundView`** (SwiftUI) checks **`WgpuiBackgroundBridge.isAvailable`**:
   - If **true**: embeds **`Representable`** → **`WgpuiBackgroundUIView`** (UIKit, `CAMetalLayer`), which creates Rust WGPUI state and drives a `CADisplayLink` to render each frame.
   - If **false**: shows **`Color.black`** (full-screen black).

So a black screen in production can mean either:

- **A)** Bridge reports “not available” → we never create the Metal/WGPUI view and show `Color.black`, or  
- **B)** Bridge is available but **`wgpui_ios_background_create`** returns **null** → we create the Metal view but never start the display link; the view’s `backgroundColor = .black` is what you see.

### 1.2 Symbol loading (why “available” can be false in Release)

WGPUI is **not** linked at compile time from Swift. The Swift bridge loads the Rust C symbols at **runtime** via **`dlsym`**:

- **`WgpuiBackgroundBridge`** (`WgpuiBackgroundBridge.swift`):
  - `wgpuiHandle`: first tries **`dlopen(nil, RTLD_NOW)`** (main executable), then falls back to **`Autopilot.debug.dylib`** (only present in some debug setups).
  - Loads: `wgpui_ios_background_create`, `_render`, `_resize`, `_destroy`, plus tap/login/email FFI.
  - **`isAvailable`** = all of create/render/resize/destroy are non-nil.

Rust symbols come from the **statically linked** `libopenagents_client_core.a` (device: `ios-arm64`, simulator: `ios-arm64_x86_64-simulator`). The linker is given **`-u _wgpui_ios_background_create`** and **`-force_load …/libopenagents_client_core.a`** so the .o that defines the WGPUI FFI is included. For Archive (device), SDK-conditional **`OTHER_LDFLAGS[sdk=iphoneos*]`** and **`LIBRARY_SEARCH_PATHS[sdk=iphoneos*]`** point at the device slice only.

If the **symbol table is stripped** in the Release/Archive binary, **`dlsym(mainHandle, "wgpui_ios_background_create")`** returns **nil** → **`isAvailable`** is false → **Color.black**.

### 1.3 What happens when create() is called

When the bridge is available, **`WgpuiBackgroundUIView.layoutSubviews`** calls:

- **`WgpuiBackgroundBridge.create(layerPtr:width:height:scale:)`**  
  → Rust **`wgpui_ios_background_create`** in **`crates/wgpui/src/platform.rs`**.

Rust **`IosBackgroundState::new`**:

1. Creates **`wgpu::Instance`** (Metal backend).
2. **`create_surface_unsafe(CoreAnimationLayer(layer_ptr))`**.
3. **`pollster::block_on(instance.request_adapter(...))`**.
4. **`pollster::block_on(adapter.request_device(...))`**.
5. Configures surface, creates renderer, returns **`Ok(Box::new(Self { ... }))`** or **`Err(...)`**.

All of this runs on the **main thread** (called from `layoutSubviews`). If **`create()`** returns **null** (Rust returns `Err`), Swift never starts the **`CADisplayLink`**; the UIView stays with **`backgroundColor = .black`** → black screen (case B above).

---

## 2. What was tried (and did not fix the black screen)

### 2.1 SDK-conditional linking (Archive linker error)

**Problem:** Archive failed with “Building for 'iOS', but linking in object file … built for 'iOS-simulator'”.

**Change:** In **`Autopilot.xcodeproj/project.pbxproj`**, for the Autopilot target (Debug and Release):

- **`OTHER_LDFLAGS[sdk=iphoneos*]`**: force_load **`…/ios-arm64/libopenagents_client_core.a`**.
- **`OTHER_LDFLAGS[sdk=iphonesimulator*]`**: force_load **`…/ios-arm64_x86_64-simulator/libopenagents_client_core_sim.a`**.
- **`LIBRARY_SEARCH_PATHS[sdk=iphoneos*]`** / **`[sdk=iphonesimulator*]`** set so each SDK uses only its slice.

**Result:** Archive and TestFlight install succeed; app no longer crashes at launch. **Black screen remained.**

### 2.2 Disabling symbol stripping in Release

**Hypothesis:** In Release/Archive, the binary is stripped and **dlsym** can’t find the WGPUI symbols → **isAvailable** false → **Color.black**.

**Change:** For the **Autopilot** target, **Release** configuration only:

- **`STRIP_INSTALLED_PRODUCT = NO`**

**Result:** Black screen **still** occurs on TestFlight. So either:

- Stripping was not the cause, or  
- Another mechanism (e.g. dead code stripping, or symbols still not exported) removes the symbols, or  
- The real issue is (B): symbols are found but **create()** returns null or never returns on device.

---

## 3. Root cause analysis (current best hypotheses)

### 3.1 Hypothesis A: Symbols still missing in Release/device

- **dlsym** might still not see the symbols (e.g. different strip/dead-code behavior for device vs simulator, or export list).
- **Check:** On a device build (or an exported .ipa from Archive), run:
  - `nm -gU <path-to-Autopilot-binary>` (or equivalent for the main executable inside the .app) and confirm **`_wgpui_ios_background_create`** (and other `_wgpui_ios_background_*`) are present.
- **If missing:** Consider an explicit exported-symbols list or linker flags that keep only these symbols without hiding the rest of the app (e.g. avoid **EXPORTED_SYMBOLS_FILE** as a full whitelist if it would hide **oa_client_core_*** and other required symbols).

### 3.2 Hypothesis B: `wgpui_ios_background_create` fails or blocks on device (main thread + Metal)

- **create()** is invoked from **main thread** (**layoutSubviews**). Inside Rust, **`pollster::block_on`** is used twice (request_adapter, request_device). Blocking the main thread on iOS is sensitive:
  - Simulator vs device: Metal/GPU stack can differ; adapter/device creation might need main-thread or not, or might deadlock if main is blocked.
  - If **block_on** deadlocks on device, **create()** never returns → no display link → black view.
  - If **request_adapter** or **request_device** fails on device (e.g. Metal limits, sandbox, or different defaults), Rust returns **Err** → **create()** returns null → again black view.
- **eprintln!** in Rust is not visible in TestFlight/crash logs; so we don’t know if create failed with an error or hung.
- **Check:** Add a **diagnostic path** that doesn’t rely on Rust logs:
  - e.g. when **create()** returns null, set a flag and show a simple **Text("WGPUI init failed")** or a different background color in Swift so TestFlight users (or you on a device build) can confirm we’re in case B.

### 3.3 Hypothesis C: Metal / wgpu behavior on real device

- Surface configuration, format, or present mode might differ on device and cause **create_surface_unsafe** or **surface.configure** to fail.
- **Check:** Same as B: surface failure path in Rust returns null; expose that in UI or logs to confirm.

---

## 4. Recommended next steps

1. **Confirm which case (A vs B)**  
   - Add a **visible fallback** when **`WgpuiBackgroundBridge.create(...)`** returns **nil** (e.g. “WGPUI failed to start” or a distinct background color) so that on TestFlight/device you can tell “bridge available but create failed” vs “bridge not available”.
   - Optionally, when **`isAvailable`** is false, show a different message (e.g. “WGPUI symbols not found”) so we can distinguish A vs B without debugger.

2. **Verify symbols in the shipped binary**  
   - From an Archive, export the .app or .ipa and inspect the main executable:
     - `nm -gU` (or your project’s strip/export settings) and confirm **`_wgpui_ios_background_create`** (and other WGPUI symbols) are present in the device build.

3. **Move WGPUI init off the main thread**  
   - **create()** currently runs on the main thread and uses **pollster::block_on**. Options:
     - Create the **IosBackgroundState** (adapter/device/surface) on a **background queue**, then pass the resulting state (or a token) back to the main thread and start the **CADisplayLink** there; or
     - Make the Rust side **async** and have Swift call it from a task and only attach the view/display link when init completes.
   - This avoids main-thread block_on and may fix device-only deadlock or Metal quirks.

4. **Add minimal in-app or crash diagnostics**  
   - e.g. **os_log** or a small file in the container when **isAvailable** is false vs when **create()** returns null, so a TestFlight install can be correlated with “symbols missing” vs “create failed” (and later with crash logs if you add that).

5. **Revisit stripping/export only if symbols are missing**  
   - If **nm** (or equivalent) shows WGPUI symbols are absent from the Release/device binary, then consider:
     - **STRIP_STYLE** (e.g. keep global symbols), or  
     - A **symbol file or linker flags** that preserve **only** the WGPUI symbols without turning **EXPORTED_SYMBOLS_FILE** into a full whitelist that would hide **oa_client_core_*** and break the app.

---

## 5. Code references

| What | Where |
|------|--------|
| Swift bridge (dlsym, isAvailable, create) | `apps/autopilot-ios/Autopilot/Autopilot/WgpuiBackgroundBridge.swift` |
| Swift view (Color.black vs Representable, layoutSubviews → create) | `apps/autopilot-ios/Autopilot/Autopilot/WgpuiBackgroundView.swift` |
| Root UI | `apps/autopilot-ios/Autopilot/Autopilot/ContentView.swift` |
| Rust FFI + IosBackgroundState::new (block_on, Metal) | `crates/wgpui/src/platform.rs` (ios module) |
| Xcode build settings (OTHER_LDFLAGS, LIBRARY_SEARCH_PATHS, STRIP_INSTALLED_PRODUCT) | `apps/autopilot-ios/Autopilot/Autopilot.xcodeproj/project.pbxproj` (Autopilot target, Debug/Release) |
| WGPUI on iOS overview | `apps/autopilot-ios/docs/WGPUI-IOS.md` |
| Building / XCFramework | `apps/autopilot-ios/docs/BUILDING.md` |

---

## 6. Summary

- **Black screen on TestFlight** = either **WgpuiBackgroundBridge.isAvailable** is false (symbols not found → **Color.black**) or **create()** returns null / never returns (Metal view created but no render → black UIView).
- **Already done:** SDK-conditional linking so Archive uses the device slice; **STRIP_INSTALLED_PRODUCT = NO** for Release. Black screen persists.
- **Next:** (1) Add visible or logged diagnostics to distinguish “symbols missing” vs “create failed”; (2) Inspect the Archive/device binary for WGPUI symbols; (3) Move WGPUI init off the main thread (async or background queue) to avoid **block_on** on main and device-only Metal issues.
