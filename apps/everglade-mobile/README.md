# Everglade for mobile

The Everglade app for iOS and Android, bundle id `com.openagents.everglade`.
It opens on the Golemancers' Workshop, the same scene as
[Everglade for desktop](../../everglade/docs/desktop-app-spec.md): the temple
built from Aethyr's architecture kit, drawn by Aethyr's shaders, with your
character and the shared walking controller.

Status: **iOS runs in the simulator and Android runs in the emulator.** No real device has run either.

## Run it on iOS

```sh
apps/everglade-mobile/build.sh --open
```

That builds `build/ios/Everglade.app` for the simulator, installs it and
opens it. If no simulator device exists it makes an iPhone 17 Pro named
"Everglade iPhone". It needs Xcode with an iOS simulator runtime.

## Run it on Android

```sh
apps/everglade-mobile/android/build.sh --open
```

That builds an APK, starts an emulator if no device is connected, and opens
the app. See [`android/README.md`](android/README.md) for what it needs.

## Controls

| Touch | What it does |
| --- | --- |
| Drag on the left side | Move. A small push walks, a full push runs |
| Drag on the right side | Look |
| Tap on the right side | Jump |
| Pinch (on Android, two fingers on the right side) | Zoom |

The app turns with the phone, upright or sideways.

## What is shared with the desktop app

Nothing in the scene is written twice.

| Piece | Where |
| --- | --- |
| The scene: walking, camera, collision, the text | [`apps/everglade-desktop/workshop.coax`](../everglade-desktop/workshop.coax) |
| The temple's parts and where they stand | [`everglade/workshop/temple.coax`](../../everglade/workshop/temple.coax), from [`aethyr/arch/`](../../aethyr/arch/README.md) |
| The temple's shaders | [`everglade/workshop/gpu/temple.coax`](../../everglade/workshop/gpu/temple.coax), emitted by `coax metal` for iOS and `coax spirv` for Android |
| Your character | [`apps/everglade-desktop/model.coax`](../everglade-desktop/model.coax) and Everglade II's pack |
| The GPU side of the Apple hosts: the pack, the place, the model | [`apps/everglade-desktop/Sources/Engine.swift`](../everglade-desktop/Sources/Engine.swift) |
| The fonts' atlas, drawn | [`apps/everglade-desktop/Sources/Faces.swift`](../everglade-desktop/Sources/Faces.swift) |

What is iOS's own is [`ios/Sources/main.swift`](ios/Sources/main.swift):
a full-screen Metal view, touch, the safe area, and a thread with a deep
stack (the web's character code passes megabytes by value, and iOS gives the
main thread one megabyte).

What is Android's own is [`android/host.c`](android/host.c): the same in C
over Vulkan, which also does what `Engine.swift` does for Metal. Two shaders
are written by hand for each GPU (the strokes and your model); the Android
copies are in [`android/shaders/`](android/shaders).

The phone has no Next issue panel: that panel runs a `claude` process, which a
phone can't do.

## The icon

Both apps carry the big E of the Everglade logo. The sources and how to draw
them again are in [`icon/`](icon/README.md).

## Not done

- **A real device.** The build targets the simulator. A device build needs a
  signing team and a provisioning profile, which are the owner's.
- **Android's own list** is in [`android/README.md`](android/README.md).
- **App size.** The bundle carries Everglade II's whole 61 MB pack for the
  character's parts. A phone build should carry only those parts.
- **The deep-stack thread** is 512 MB of address space. It works in the
  simulator; whether a device grants it is untested.
- **A launch screen.**
- **TestFlight.** Nothing is uploaded.
