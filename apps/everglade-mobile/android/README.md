# Everglade for Android (not built)

The plan, so the iOS and Android apps stay one app.

| Piece | iOS today | Android |
| --- | --- | --- |
| The scene, the temple, the character | Coax compiled to C | The same C, built with the NDK for `arm64-v8a` |
| The temple's shaders | `coax metal` | `coax spirv`, the same source |
| The GPU | Metal | Vulkan |
| The host | Swift, UIKit | A `NativeActivity` in C: a window, touch, the safe area |
| The GPU side of the host | `Engine.swift` | New: a Vulkan version that reads the same mesh, records and draws |
| The character's shader | `Model.metal`, a hand copy | Needs writing for Vulkan, or the web's own Coax shader emitted |
| Fonts' atlas | CoreText | Needs a rasterizer; FreeType through the NDK, or an atlas baked at build time |

The package name is `com.openagents.everglade`.

The work that iOS did not need: a Vulkan host. Aethyr has one for Linux
(`apps/aethyrforge-desktop/linux-native*.coax`, `coax/platform/wayland-native.c`),
which draws Bitcoin Lab. Android would reuse its pass host and swap the
window.

Nothing here is built or tested.
