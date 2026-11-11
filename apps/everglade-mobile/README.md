# Everglade for mobile

The Everglade app for iOS and Android, bundle id `com.openagents.everglade`.
It opens on a list of worlds ([spec](../../everglade/docs/app-scenes-spec.md)),
the same as [Everglade for desktop](../../everglade/docs/desktop-app-spec.md):

| World | On iOS | On Android |
| --- | --- | --- |
| Everglade (the main flow) | Yes: the web's realm, run natively (Character Create, then the glade) | Not yet: a card says where it runs on the web |
| The Golemancers' Workshop | Yes: the temple, your character, the shared walking controller | Yes |
| The Archery Range | Yes: the web's realm, run natively ([spec](../../everglade/docs/archery-range-spec.md)) | Not yet: a card |
| Everglade Experimental | Yes: the web's realm, run natively | Not yet: a card |

These apps are an experiment beside the main experience, which is the web app.

Status: **on iOS the four worlds ran in the iPhone 17 Pro simulator on
2026-11-10** (shots read: the range with a draw, an arrow in a face and the
tally counting; Character Create and the glade; Experimental). The Workshop
ran in the Android emulator before the list of worlds was added. No real
device has been checked by hand.

## The web's realms on iOS

The three realms run through Aethyr's page host
([`aethyr/host/apple/`](../../aethyr/host/apple/page-host.h)), the same host
and the same table of rows
([`Realms.swift`](../everglade-desktop/Sources/Realms.swift)) as the Mac app
([how](../../everglade/docs/app-scenes-spec.md#4-bringing-the-webs-worlds-into-the-apps)).
What is the phone's own is in [`ios/Sources/Pages.swift`](ios/Sources/Pages.swift):

- **Touch.** Every finger goes to the realm as a browser's touch does, each
  with its own number, so the realm's own touch controls work: hold the bow's
  slot (Attack) to draw, drag to aim, lift to loose; drag elsewhere to look;
  tap a slot or a button. A touch that begins on **Worlds** is the app's.
- **The screen.** The realm fills the screen inside the notch, the corners
  and the home bar.
- **The keyboard** comes up while the realm takes text.
- **Code.** iOS loads code only from a framework, so each realm's library is
  `Frameworks/NAME.framework`.
- **Memory.** A phone's layers are 512 texels a side, the tier
  `aethyr/render/texture-format.coax` gives a phone, and the page host lets
  go of the pack's bytes once they are on the GPU. Measured in the simulator
  with `footprint`: Experimental and the range hold 466 MB (719 MB at the
  peak, while the 286 MB pack is inflated); Everglade holds 195 MB.
- **Sound** is the Mac app's: Aethyr's Apple audio host
  (`aethyr/host/apple/PageAudio.swift`). It mixes with what is playing and
  the ring switch silences it. In a shot it is rendered, never played: the
  range's run said the bow's draw and release, the arrow's flight and hit,
  and your steps. Nobody has heard it on a phone yet.
- **What is kept** (your character, your place, your settings) is in the
  app's own Application Support folder.
- **Who it counts as.** These builds go to the team, so their sessions are
  marked as a driven browser's are and never count as visitors.

What the realms don't do on a phone yet:

- **Walking in the range and Experimental.** The web's realm has no thumb
  stick on those pages (only the Druid's grove and Everglade have one), so
  you can shoot and look but not walk.
- **Worlds** sits over the middle of the realm's top panel on an upright phone.
- **Opening a realm** holds the screen still for a few seconds while its pack
  loads.

A shot drives fingers from the launch arguments, since nothing outside the
simulator can touch its screen (`Pages.swift`, `Script`):

```sh
xcrun simctl launch booted com.openagents.everglade --offline --scene 3 \
  --do "wait:6;down:1:110,721;drag:1:103,682:0.8;wait:1.5;up:1:103,682;wait:2;mark:done"
xcrun simctl io booted screenshot range.png
```

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
be higher than the last one uploaded (3 went up on 2026-11-10, the first with
the web's realms). It needs
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

In the Workshop (a realm's touches are its own, above):

| Touch | What it does |
| --- | --- |
| Drag on the left side | Move. A small push walks, a full push runs |
| Drag on the right side | Look |
| Tap on the right side | Jump |
| Two fingers on the right side, pinched | Zoom |
| Turn the phone (iOS) | Look, while the Gyro switch under the title is on. Tap the switch to turn it off |

Both thumbs work at once: you can walk and look together.

The app turns with the phone, upright or sideways.

## What is shared with the desktop app

Nothing in the scene is written twice.

| Piece | Where |
| --- | --- |
| The list of worlds, and which one gets each key and touch | [`apps/everglade-desktop/scenes.coax`](../everglade-desktop/scenes.coax) |
| The Workshop: walking, camera, collision, the text | [`apps/everglade-desktop/workshop.coax`](../everglade-desktop/workshop.coax) |
| The temple's parts and where they stand | [`everglade/workshop/temple.coax`](../../everglade/workshop/temple.coax), from [`aethyr/arch/`](../../aethyr/arch/README.md) |
| The temple's shaders | [`everglade/workshop/gpu/temple.coax`](../../everglade/workshop/gpu/temple.coax), emitted by `coax metal` for iOS and `coax spirv` for Android |
| Your character | [`apps/everglade-desktop/model.coax`](../everglade-desktop/model.coax) and Everglade II's pack |
| The GPU side of the Apple hosts: the pack, the place, the model | [`apps/everglade-desktop/Sources/Engine.swift`](../everglade-desktop/Sources/Engine.swift) |
| The fonts' atlas, drawn | [`apps/everglade-desktop/Sources/Faces.swift`](../everglade-desktop/Sources/Faces.swift) |

What is iOS's own is [`ios/Sources/main.swift`](ios/Sources/main.swift)
and [`ios/Sources/Pages.swift`](ios/Sources/Pages.swift):
a full-screen Metal view, touch, the safe area, the realms' pages, and a thread with a deep
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

- **A real phone has not run it.** Builds 1 to 3 went to TestFlight without one.
- **Android's own list** is in [`android/README.md`](android/README.md).
- **App size.** The bundle carries Everglade II's 59 MB pack, Experimental's
  92 MB pack and 28 MB of the bars' pictures.
- **The deep-stack thread** is 512 MB of address space. It works in the
  simulator; whether a device grants it is untested.
- **A launch screen.**
