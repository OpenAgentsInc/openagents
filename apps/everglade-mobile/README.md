# Everglade for mobile

The Everglade app for iOS and Android, bundle id `com.openagents.everglade`.
It opens on a list of worlds ([spec](../../everglade/docs/app-scenes-spec.md)),
the same as [Everglade for desktop](../../everglade/docs/desktop-app-spec.md):

| World | In the app |
| --- | --- |
| Everglade (the main flow) | Not yet: a card says where it runs on the web |
| The Golemancers' Workshop | Yes: the temple, your character, the shared walking controller |
| The Archery Range | Yes: shoot for points on foot, from horseback and dragonback ([spec](../../everglade/docs/archery-range-spec.md)) |
| Everglade Experimental | Not yet: a card says where it runs on the web |

These apps are an experiment beside the main experience, which is the web app.

Status: **the Workshop ran in the iOS simulator and the Android emulator
before the list of worlds was added. The list and the Archery Range have not
been built or run on either yet** (written on Linux; the Coax is checked).
No real device has run any of it.

## Run it on iOS

```sh
apps/everglade-mobile/build.sh --open
```

That builds `build/ios/Everglade.app` for the simulator, installs it and
opens it. If no simulator device exists it makes an iPhone 17 Pro named
"Everglade iPhone". It needs Xcode with an iOS simulator runtime.

## Send it to TestFlight

```sh
apps/everglade-mobile/ios/release.sh BUILD_NUMBER
```

That builds for iPhone, signs with the team's Apple Distribution identity,
and uploads to the App Store Connect app "Everglade". The build number must
be higher than the last one uploaded (2 went up on 2026-11-10). It needs
`~/work/.secrets/appstoreconnect.env`. Apple then processes the build:

```sh
apps/everglade-mobile/ios/release.sh status BUILD_NUMBER
```

The script does not add the build to a tester group. Do that in
[App Store Connect](https://appstoreconnect.apple.com/apps/6833092875/testflight/ios),
under TestFlight.

## Run it on Android

```sh
apps/everglade-mobile/android/build.sh --open
```

That builds an APK, starts an emulator if no device is connected, and opens
the app. See [`android/README.md`](android/README.md) for what it needs.

## Controls

Tap a world on the list to open it. **Worlds**, under a world's title, comes back.

| Touch | What it does |
| --- | --- |
| Drag on the left side | Move. A small push walks, a full push runs |
| Drag on the right side | Look |
| Tap on the right side | Jump |
| Two fingers on the right side, pinched | Zoom |
| Turn the phone (iOS) | Look, while the Gyro switch under the title is on. Tap the switch to turn it off |

Both thumbs work at once: you can walk and look together.

The Archery Range adds its own buttons: hold **Draw**, drag to aim and let go
to loose; **Foot**, **Horse** and **Dragon** change how you shoot; on the
dragon, **Up** and **Down**.

The app turns with the phone, upright or sideways.

## What is shared with the desktop app

Nothing in the scene is written twice.

| Piece | Where |
| --- | --- |
| The list of worlds, and which one gets each key and touch | [`apps/everglade-desktop/scenes.coax`](../everglade-desktop/scenes.coax) |
| The Workshop: walking, camera, collision, the text | [`apps/everglade-desktop/workshop.coax`](../everglade-desktop/workshop.coax) |
| The Archery Range: the bow, the mounts, scoring | [`apps/everglade-desktop/range.coax`](../everglade-desktop/range.coax), [`everglade/range/`](../../everglade/range) |
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

- **A real phone has not run it.** Build 1 went to TestFlight without one.
- **Android's own list** is in [`android/README.md`](android/README.md).
- **App size.** The bundle carries Everglade II's whole 61 MB pack for the
  character's parts. A phone build should carry only those parts.
- **The deep-stack thread** is 512 MB of address space. It works in the
  simulator; whether a device grants it is untested.
- **A launch screen.**
