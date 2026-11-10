# Everglade for Android

The Android app, package `com.openagents.everglade`. It opens on the same
Golemancers' Workshop as the iOS and desktop apps.

Status: **runs in the Android emulator** (Android 15, arm64) at 60 frames a
second. No real phone has run it.

## Run it

```sh
apps/everglade-mobile/android/build.sh --open
```

That builds `apps/everglade-mobile/build/android/everglade.apk`, then installs
and opens it on the device or emulator `adb` sees. If none is up, it starts an
emulator named `everglade_api35`, making it first if needed.

It needs the Android SDK at `~/Library/Android/sdk` (or `ANDROID_HOME`) with
platform `android-35`, build-tools `35.0.0`, an NDK and, for the emulator, the
`google_apis` arm64 system image for Android 15. It needs a Mac, because
CoreText draws the fonts' atlas at build time. There is no Gradle and no Java:
the app is one native library.

## What is here

| File | What |
| --- | --- |
| [`host.c`](host.c) | The host: a `NativeActivity`, a Vulkan swapchain, touch, and the GPU side that `Engine.swift` is on iOS |
| [`shaders/`](shaders) | The strokes' and your model's shaders as GLSL: the Vulkan versions of `Workshop.metal` and `Model.metal` |
| [`bake-faces/`](bake-faces/main.swift) | Writes the fonts' atlas to a file at build time, from the same drawing code the Apple hosts use |
| [`AndroidManifest.xml`](AndroidManifest.xml) | The package: Android 11 and up, Vulkan 1.1 |
| [`build.sh`](build.sh) | Builds, packs and signs the APK with the NDK, `aapt2` and `apksigner` |

The scene, the temple and your character are the same Coax as everywhere
else, compiled to C for `arm64-v8a`. The temple's shaders are the same Coax
source, emitted by `coax spirv` where iOS uses `coax metal`.

## What a phone must have

- Android 11 or later (the picture decoder the app uses arrived then).
- Vulkan 1.1 with 64-bit integers in shaders (`shaderInt64`), which Aethyr's
  emitted shaders use. The app stops with a message in the log when the GPU
  lacks it. Most recent phones have it; some older Mali GPUs don't.
- arm64.

## Not done

- **A real phone.** Only the emulator has run it. A phone's GPU may differ:
  the 64-bit integers above, and memory for the 512 MB deep stack the
  character's code needs.
- **A release key.** The APK is signed with the debug key. The Play Store
  needs the owner's key and an AAB.
- **Turning the phone while the app runs** is written (the swapchain is
  remade) and untested.
- **Pinch** is written (a second finger on the look side) and untested: the
  emulator's scripted input has one finger.
- **An icon.** The app shows Android's default.
- **App size.** The APK is 64 MB, nearly all of it Everglade II's whole pack.
