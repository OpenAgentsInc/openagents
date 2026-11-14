# Arcade audit 3: design and component library

Tracking issue: #13607. This doc specifies Arcade's visual design and
component library precisely enough to rebuild it, and then maps each piece
onto the OpenAgents component kit for OpenAgents iOS. The owner's rule for
the port: bring the whole design and component library over, but replace
Arcade's cyan with white and grayscale, as the current app uses.

Part A describes Arcade as it is. Part B is the adaptation. Arcade paths are
relative to `/Users/christopherdavid/work/arcade`; OpenAgents paths are
relative to the openagents repository root.

Marks used in this doc:

- **Verified**: read in the code or measured from the file.
- **Inferred**: concluded from the code but not run or seen on a device.
- **Dead**: in the repository but never reached from a live screen.
- **Ignite**: unchanged boilerplate from the Ignite template. The template's
  `DemoScreen`, `docs/ignite` folder, demo icons and demo images are
  boilerplate. This doc names them once and does not describe them further.

---

## Part A: Arcade as it is

### A1. Design language

Arcade is a dark, single-theme app. Every screen sits on near-black
(`#030303`). Structure comes from 1-pixel cyan hairlines, not from filled
surfaces or shadows. Surfaces that need a fill use a faint translucent cyan
wash (11% or 5% alpha) over the black, so they read as tinted glass. Accents
(timestamps, links, labels, placeholders) use mid-range cyan. Primary actions
use a solid mid cyan. Send buttons are solid round cyan discs. Text is
off-white.

Corners are mostly square or slightly rounded (4 to 8 px). Message bubbles
are fully square. Only the composer pill, avatars and round icon buttons are
round.

The "Arwes" sci-fi frame look (corner brackets, animated borders, a neon
city backdrop, glowing gradients) exists in the code, but only fragments of
it ship (verified):

- The neon city image appears only behind the logged-out Home screen.
- The animated Frame, ArwesButton and ArwesTextInput appear only on the
  unreachable Demo screen and in a tab bar that is no longer used.
- The glow gradient, spotlight and blur splash are dead.

So the shipped design is "black glass with cyan hairlines" plus a cyberpunk
title screen. The HUD look was tried and backed out. Git shows the Frame tab
bar added in `d520b0f` (#188) and removed from use in `756427a` (#217).

The type is Space Grotesk, a geometric grotesk, for everything. Protomolecule,
a display face, is used only for the "arcaDE" wordmark.

Motion is quiet. Fades are 200 to 800 ms, lists stagger in, swipe-to-reply
has a progress donut, and a long-press menu blurs the screen. Haptics are
almost absent.

There is no light theme. `app.json` sets `userInterfaceStyle: "dark"`, and
every colour is hard-coded dark (verified).

### A2. Tokens

#### A2.1 Palette (`app/theme/colors.ts`, verified)

| Name | Value | Used for (verified unless noted) |
| --- | --- | --- |
| `almostBlack` | `#030303` | Screen background (`colors.background`) |
| `black` | `#000000` | Outer screen view, sheets, Faerie loading row |
| `white` | `"white"` | Body text on screens that override `text` |
| `gray` | `#7B7C7F` | List-row times and last-message text |
| `neutral100` | `#FFFFFF` | `colors.text`; Ignite button fill (default preset) |
| `neutral200` | `#F4F2F1` | Ignite TextField fill, Toggle off (Ignite defaults, overridden on screens) |
| `neutral300` | `#D7CEC9` | Ignite filled button, Toggle off border |
| `neutral400` | `#B6ACA6` | `colors.border` |
| `neutral500` | `#978F8A` | Ignite Card reversed border |
| `neutral600` | `#564E4A` | `colors.textDim` |
| `neutral700` | `#3C3836` | Ignite reversed button pressed |
| `neutral800` | `#191015` | Ignite reversed button, Card shadow, error stack box |
| `neutral900` | `#000000` | Not used directly |
| `primary100`–`primary600` | `#F4E0D9`, `#E8C1B4`, `#DDA28E`, `#D28468`, `#C76542`, `#A54F31` | Ignite orange. Not used by any live screen |
| `secondary100`–`secondary500` | `#DCDDE9`, `#BCC0D6`, `#9196B9`, `#626894`, `#41476E` | `secondary500` is the Ignite Toggle on-fill (overridden on screens) |
| `accent100`–`accent500` | `#FFEED4`, `#FFE1B2`, `#FDD495`, `#FBC878`, `#FFBB50` | Ignite. Not used by live screens |
| `cyan50` | `#ecfeff` | Not used |
| `cyan100` | `#cffafe` | Send-arrow glyph, contact action icons, private-channel card border |
| `cyan200` | `#a5f3fc` | Not used |
| `cyan300` | `#67e8f9` | Not used |
| `cyan400` | `#22d3ee` | Header back icon, profile name, author name, listing labels, drawer buttons, FAB |
| `cyan500` | `#06b6d4` | Primary button fill, timestamps, links, placeholders, reply bar, borders of ghost buttons and sheets |
| `cyan600` | `#0891b2` | Round send button, pressed primary, link-preview description, npub text |
| `cyan700` | `#0e7490` | Paperclip icon, attachment close icon, link-preview domain, UserFeed second line |
| `cyan800` | `#155e75` | Tab-bar top border, inactive tab icons, Toggle track, sheet placeholders, changelog dividers |
| `cyan900` | `#164e63` | `colors.separator`: bubble borders, field borders, composer top border, section boxes |
| `cyan950` | `#083344` | Swipe icon disc, ListingItem fill |
| `angry100` | `#F2D6CD` | `colors.errorBackground` |
| `angry500` | `#C03403` | `colors.error` |
| `overlay20` | `rgba(69, 210, 255, 0.11)` | Others' bubbles, composer, fields, cards, reply bar (line 54) |
| `overlay50` | `rgba(69, 210, 255, 0.05)` | Own bubbles, drawer overlay |
| `overlay80` | `rgba(0, 0, 0, 0.5)` | Avatar-picker scrim, some toggles |

The cyan50–950 ramp is Tailwind's `cyan` scale. Despite their names,
`overlay20` and `overlay50` are 11% and 5% alpha.

#### A2.2 Semantic roles (`colors.ts` lines 80–100, verified)

| Role | Value |
| --- | --- |
| `text` | `neutral100` (#FFFFFF) |
| `textDim` | `neutral600` (#564E4A) |
| `background` | `almostBlack` (#030303) |
| `border` | `neutral400` (#B6ACA6) |
| `tint` | `#5BC6E0` (line 90; not on the cyan ramp) |
| `separator` | `cyan900` (#164e63) (line 94) |
| `error` | `angry500` |
| `errorBackground` | `angry100` |
| `transparent` | `rgba(0, 0, 0, 0)` |

#### A2.3 Literals outside the theme (verified by search)

| Literal | Where |
| --- | --- |
| `#232324` | List-row avatar border and dividers (ChannelItem, DirectMessageItem, AIChannelDetail) |
| `#7B7C7F` | Row time and last message (duplicates `gray`) |
| `#666` / `#000` | Unread badge pill and its text (commented out in live rows) |
| `#1C1C1D` | Faerie (AI) message row fill |
| `#8EA960` | AnimatedTyping cursor (dead) |
| `#8E8E92` | CopyButton icon (dead) |
| `#1c1c1e`, `#3b3b3d`, `#888`, `#999` | TrainAI cards (unreachable) |
| `"cyan"`, `rgb(0,248,248)` | BackgroundGradient sweep (dead) |
| `#00ffff` | Text shadow on the "arcaDE" wordmark |
| `rgba(0,42,59,0.7)` | Spotlight gradient (dead) |
| `rgba(0,0,0,0.75)` | CityBackground dimmer |
| `rgba(0,0,0,0.5)` | Ghost buttons on Home |
| `rgba(255,255,255,0.5)` | Secondary text in manager and contact rows; swipe donut |
| `rgba(255,255,255,0.1)` | TouchableFeedback press fill |
| `rgba(255,255,255,0.75)` | BlurredPopup menu fill |

#### A2.4 Typography (`app/theme/typography.ts`, `app/components/Text.tsx`, verified)

Families:

| Key | Face | Source |
| --- | --- | --- |
| `primary` | Space Grotesk 300, 400, 500, 600, 700 | `@expo-google-fonts/space-grotesk` (npm). No TTF in `assets/` |
| `heading` (Text preset only) | Protomolecule | `assets/fonts/Protomolecule.otf` (109,184 bytes) |
| `secondary` | Helvetica Neue on iOS, `sans-serif` on Android | System |
| `code` | Courier on iOS, `monospace` on Android | System |

Sizes (font size / line height, px):

| Key | Size |
| --- | --- |
| `xxl` | 36 / 44 |
| `xl` | 24 / 34 |
| `lg` | 20 / 32 |
| `md` | 18 / 26 |
| `sm` | 16 / 24 (default) |
| `xs` | 14 / 21 |
| `xxs` | 12 / 18 |

Text presets:

| Preset | Spec |
| --- | --- |
| `default` | `sm`, Space Grotesk normal (400), colour `text` |
| `bold` | `default` + 700 |
| `heading` | `xxl`, 700, Protomolecule |
| `subheading` | `lg`, 500 |
| `formLabel` | `default` + 500 |
| `formHelper` | `default` + 400 |

Text supports RTL through `writingDirection`. The wordmark on Home overrides
the heading preset to 75 px, line height 100, letter spacing 4, with a
`#00ffff` text shadow of radius 4.

#### A2.5 Spacing (`app/theme/spacing.ts`, verified)

| Key | px |
| --- | --- |
| `micro` | 2 |
| `tiny` | 4 |
| `extraSmall` | 8 |
| `small` | 12 |
| `medium` | 16 |
| `large` | 24 |
| `extraLarge` | 32 |
| `huge` | 48 |
| `massive` | 64 |

Screens mostly use literal values: 8, 12 and 16 for padding, and 24 for the
composer's side padding.

#### A2.6 Radii, borders, shadows, opacity (verified across screens)

| Token (named here) | Value | Where |
| --- | --- | --- |
| Radius 0 | 0 | Message bubbles |
| Radius 1 | 1 | Reply quote |
| Radius 4 | 4 | Ignite Button and TextField, section boxes, relay rows, ListingItem, attachment thumbnail, channel tile image |
| Radius 5 | 5 | Long-press menu |
| Radius 6 | 6 | Channel tiles (Card override) |
| Radius 8 | 8 | Primary buttons, "Arcade field", Add/Remove pills, sheet fields, Toggle override |
| Radius 16 | 16 | Ignite Card, Switch track |
| Radius 20 | 20 | Composer pill, swipe disc, close-button press |
| Full | 100 or half the size | Avatars, send, FAB, round icon buttons |
| Hairline | 1 px | All borders and dividers |
| Avatar ring | 0.6 px `#232324` | List-row avatars |
| Profile ring | 2 px black | User-screen avatar |
| Reply bar | 3 px left `cyan500` | Reply quote |
| Field underline | 2 px bottom `cyan900` | CreateChannel fields |
| Shadow | `neutral800`, offset 0/12, opacity 0.08, radius 12.81, elevation 16 | Ignite Card only. Every live screen overrides it to none |

Opacity values: pressed text 0.9 (Ignite Button), row press 0.8 (ListItem),
mic press 0.8, city dimmer 0.75, and the fills in A2.1 and A2.3.

#### A2.7 Motion (verified)

| Motion | Duration and curve | Where |
| --- | --- | --- |
| `timing.quick` | 300 ms | Theme constant (rarely used) |
| Frame show and hide | `withTiming` 200 ms; scale = 1 − progress | Frame |
| Frame highlight | 0.2 → 0.7 fill opacity over 200 ms | Frame, `Frame.tsx` 149–155 |
| HUD tab fade-in | delay 500 + 50 × index, 300 ms | TabBar (dead) |
| Home list rows | fade in, delay 100 × index, 800 ms | HomeMessages |
| Swipe return | `withTiming(0)` | SwipeableItem |
| Swipe completion | spring to scale 2, opacity 0 (overshootClamping, mass 0.5) | SwipeableItem donut |
| Popup blur | 0 → 10 blur, `withTiming` 200 ms; menu fades; blur clears 200 ms after the menu | BlurredPopup |
| Press fill | to `rgba(255,255,255,0.1)` over 100 ms | TouchableFeedback |
| Reply bar | height and opacity `withTiming` | DirectMessageReply |
| Drawer | slide, open and close speed 2 | ScreenWithSidebar |
| Hamburger to arrow | spring | DrawerIconButton |
| Small spinner | 500 ms linear loop, two arcs rotate π | ActivityIndicator |
| Large spinner | outer arc on a spring loop (mass 1.5, velocity 100, damping 15) | ActivityIndicator |
| Glow pulse | blur 0 ↔ 10, 2000 ms, repeat with reverse | BackgroundGradient (dead) |
| Splash | blur +2 every 20 ms up to 300 | SplashScreen (dead) |
| Copied flash | "Copied" for 500 ms | Profile npub |
| Navigation | `slide_from_right` | AppNavigator stack |

Arcade does not check reduced motion anywhere (verified by search).

#### A2.8 Haptics (verified)

| Haptic | Where |
| --- | --- |
| `haptic("medium")` through `expo-haptics` `impactAsync` (`app/utils/haptics.ts`) | Called once: sending an AI message (`app/hooks/useSendMessage.ts`) |
| `react-native-haptic-feedback` `impactLight` | Long press that opens the BlurredPopup (`TouchablePopupHandler.tsx` line 66) |

No other screen uses haptics.

### A3. Components

Folder: `app/components/`. Each entry gives status, purpose, geometry and
states. "Drawn" means the component paints with Skia rather than composing
React Native views.

#### A3.1 Drawn components

**Frame** (`Frame/Frame.tsx`, `FrameSquare.tsx`, `AnimatedRectBorder.tsx`,
`Scaler.tsx`, `index.tsx`). Skia. Dead (used only by the Demo screen and the
unused TabBar). Verified geometry:

- Props: `color` (default `cyan400`), `borderColor` (`cyan500`),
  `strokeWidth` 1.5, `internalSquareBorderWidth` 3, `visible`,
  `highlighted` (a Skia value), `alwaysShowBackground`, `alwaysShowBorder`,
  `internalSquareSize` (default width × 0.1, line 82).
- `index.tsx` requires a numeric `width` and `height`.
- Inset: `internalSquareBorderWidth` on every side.
- Layers, back to front:
  1. A fill rect in `almostBlack`.
  2. A fill rect in `borderColor` at opacity max(scale × highlight,
     `alwaysShowBackground` ? 0.1 : 0). Highlight runs 0.2 → 0.7.
  3. A hairline box: four Skia Lines, stroke 1.5, `borderColor`. The top and
     bottom lines scale on X from their centres; the left and right lines
     scale on Y.
  4. Four corner squares: stroked rects of side `internalSquareSize`, stroke
     3, colour `color`, each placed `offset / 2` from its corner and scaled
     from that corner.
- Visibility: `withTiming` 200 ms; every part scales by 1 − progress.
- There are no cut corners. The frame is a hairline box with four square
  corner markers.

To redraw it: inset the rect by 3, stroke a 1.5-px box, then stroke four
`0.1w`-sized squares, 3 px wide, in the corners. Fill with black, plus a
tinted overlay at 10% (idle) to 70% (pressed).

**ArwesButton** (`ArwesButton/index.tsx`). Dead (Demo only). A Frame with a
Gesture.Tap: `onBegin` highlights and calls `onPressIn`; `onTouchesUp`
removes the highlight and calls `onPress`; `maxDuration` 5000.

**ArwesTextInput** (`TextInput/index.tsx`). Dead (Demo only). A Frame with
`alwaysShowBackground` and `alwaysShowBorder`. Its corner squares appear
only while the field is focused. Padding is 15 horizontal, text is `sm`, and
the selection and cursor are `neutral100`.

**ActivityIndicator** (`ActivityIndicator/`). Skia. Live: the small type
shows in empty states (Channels and others). Many screens use React Native's
ActivityIndicator instead. Geometry (verified):

- Canvas 240, aspect ratio 1, scaled by layout. Internal radius is
  (120 − 30) / 2 = 45.
- Default colour `tint`, default stroke 10.
- Small type: two 90° arcs (0–90° and 180–270°) on a square of side 45. A
  blurred filled circle of radius 45 / 2 + stroke / 2, with a BlurMask
  `solid` of radius r / 4, sits behind a black circle, which makes a glowing
  ring. The whole group rotates progress × π on a 500 ms linear loop.
- Large type: adds an outer arc at radius 45 × 1.8, turning
  counter-clockwise on a spring loop (mass 1.5, velocity 100, damping 15).

**AnimatedDonut / DonutChart** (`AnimatedDonut/`). Skia. Live, inside
SwipeableItem. A circle path of radius r, inner radius r − stroke / 2,
stroke with round caps, trimmed to `end = percentageComplete`.

**BackgroundGradient** (`BackgroundGradient.tsx`). Skia. Dead. A rounded
rect (r 20) on a canvas padded by 40, filled with a SweepGradient
`["cyan", "rgb(0,248,248)", "rgb(0,248,248)", "cyan"]`, under a BlurMask
`solid` whose blur pulses 0 ↔ 10 over 2000 ms.

**Spotlight** (`Spotlight.tsx`). Skia. Dead (commented out on Home). A
circle of radius width / 2 at (r, height / 3.2) with a radial gradient from
`rgba(0,42,59,0.7)` to black.

**BlurredPopup** (`BlurredPopup/BlurredPopupProvider.tsx`,
`TouchablePopupHandler.tsx`). Skia. Live but used once: the edit icon in
Profile's Account section, with the options "Edit Profile" and "Just a demo".

- The provider wraps the navigation stack (in AppNavigator).
- Opening snapshots the main view (`makeImageFromView`) and draws it twice
  with Skia, at blur `maxBlur / 3` and at `maxBlur` (default 10, line 62).
  The blur animates in over 200 ms (line 161).
- The pressed element is redrawn sharp at its measured position.
- The menu goes below the element, or above it if the element is within
  100 px of the bottom. It aligns left or right according to which side has
  more than 200 px free. It sits 5 px from the element.
- Menu: radius 5, fill `rgba(255,255,255,0.75)`. Items are 50 high, black
  titles, horizontal padding 10, title margins 5 vertical and 10 horizontal.
  The menu fades with `withTiming`.
- Close: the menu fades, then the blur clears 200 ms later.
- `TouchablePopupHandler`: a LongPress (minDuration 500, line 71) measures
  the element, opens the popup and fires a light haptic. A Tap runs
  `onPress`. The two gestures are combined with `Gesture.Exclusive`.

#### A3.2 Gesture and motion components

**SwipeableItem** (`SwipeableItem/index.tsx`). Live: swipe-to-reply on
channel and DM message bubbles. Own messages swipe left, others swipe right.

- Pan gesture, `activeOffsetX([-10, 10])` (line 59). Travel is clamped to
  ±100 px (`maxScrollableAmount`) in the allowed direction.
- progress = |x| / 100.
- On release: if progress is 1 it calls `onSwipeComplete`, then it springs
  back with `withTiming(0)`.
- Reply icon: a 35-px disc in `cyan950`, radius 20, holding a Lucide
  `Forward` icon (white, size 15, mirrored for left swipes). The disc is
  invisible until progress passes 0.3, then fades to 1 and moves at
  translateX / 10.
- Progress ring: DonutChart, r 17.5, stroke 3, `rgba(255,255,255,0.5)`,
  percentage = progress². When the swipe completes it springs to scale 2 and
  opacity 0.
- The donut container is absolute: top 15, height 80%, and right −20 or
  left −20.

**TouchableFeedback** (`TouchableFeedback.tsx`). A Tap gesture whose
background animates to `rgba(255,255,255,0.1)` over 100 ms.

**ReText** (`ReText/`). A non-editable animated TextInput whose text follows
a shared value. Used for the sender name in the reply bar.

**DrawerIconButton** (`DrawerIconButton.tsx`). Live (ScreenWithSidebar),
although it is marked `@demo remove-file`.

- 56 × 56 hit area holding three 2-px bars, 18 wide, 4 apart.
- While the drawer opens it springs into an arrow: the top and bottom bars
  rotate ±45°, shrink to 12 wide and shift `marginStart −11.5`. Colour moves
  from `text` to `tint`.

#### A3.3 Ignite base components as Arcade uses them

Arcade keeps Ignite's base components but overrides most of their styles on
each screen. The "Arcade override" column is what users see.

**Text** (`Text.tsx`). See A2.4.

**Button** (`Button.tsx`, Ignite).

| Part | Ignite base | Arcade override (screens) |
| --- | --- | --- |
| Box | minHeight 56, radius 4, padding 12, row, centred | Primary: `cyan500` fill, no border, radius 8, height 44–50 |
| Text | 16 / 20, Space Grotesk 500, centred | White |
| Presets | `default`: border 1 `neutral400`, fill `neutral100`, pressed `neutral200`. `filled`: `neutral300`, pressed `neutral400`. `reversed`: `neutral800`, pressed `neutral700`, white text | Ghost: transparent or `rgba(0,0,0,0.5)` fill, border 1 `cyan500` or `cyan900` |
| Pressed | Text opacity 0.9 | Primary pressed `cyan600` |
| Accessories | Left and right, margin 8 | Not used |

The default preset renders white text on a white fill, so every live screen
must override it (verified; not a visible bug only because screens override).

Round icon button (screen convention): 40 px, `cyan600` fill, radius 100.

**TextField** (`TextField.tsx`, Ignite).

| Part | Ignite base | "Arcade field" override |
| --- | --- | --- |
| Wrapper | Row, border 1 `neutral400`, radius 4, fill `neutral200` | Transparent wrapper |
| Input | Space Grotesk 16, height 24, margins 8 vertical / 12 horizontal | Height 50, border 1 `cyan900`, radius 8, fill `overlay20`, padding 16 horizontal, margin-bottom 12 |
| Label | `formLabel`, margin-bottom 8 | Same |
| Helper | margin-top 8; turns `error` with an error | Same |
| Multiline | minHeight 112 | Same |
| Accessories | 40 high, margin 8 | Eye toggle on Login |
| Disabled | Text `textDim` | Same |

**Toggle** (`Toggle.tsx`, Ignite). Variants: checkbox (24 × 24, border 2,
radius 4), radio (radius 12, dot 12) and switch (track 56 × 32, radius 16;
knob 24, radius 4; inner padding 4 at the start and end). The knob slides
with `withTiming`, and the on-fill fades in by opacity. Ignite colours: off
`neutral200` / `neutral300`, on `secondary500`, knob `neutral100` /
`neutral200`. Optional accessibility marks (line or circle, view or hidden
icons). The label can sit left or right.

Arcade override: outer border 1 `cyan900`, radius 8, fill `overlay20` (or
`overlay80`); inner track `cyan800`; knob detail radius 4 in `cyan500`.

**ListItem** (`ListItem.tsx`). Height 56, row. Left and right slots take an
icon (24, margin 16) or a component. Optional top and bottom 1-px separators
in `separator` (`cyan900`). Text has 8 vertical padding. Press opacity 0.8.

**Card** (`Card.tsx`, Ignite). Radius 16, padding 8, border 1, the shadow
in A2.6, minHeight 96, row, with heading, content and footer slots. Arcade
override (channel tiles): border 1 `cyan500` (`cyan100` for private
channels), radius 6, fill `overlay20`, no shadow.

**Header** (`Header.tsx`). Height 56 below the safe-area top; fill
`background`. Title `md` 500, either centred (absolute, padding 48
horizontal) or flexed. Left and right actions are text (`md` 500 in `tint`),
an icon (24, padding 16) or a component; an empty side is a 16-px spacer.
The icon flips in RTL. Screens set the back icon to `back` in `cyan400` and
the title in white or `cyan400`; the title colour is inconsistent between
screens (verified).

**Screen** (`Screen.tsx`). Presets `fixed`, `scroll` and `auto` (`auto`
turns scrolling on when content passes 92% of the height). Outer view black;
safe-area fill configurable; KeyboardAvoidingView with `padding` on iOS and a
`keyboardOffset`; light status bar; `useScrollToTop`.

**Icon** (`Icon.tsx`). Two sources: the PNG registry (`assets/icons/`,
tinted by `tintColor`, `resizeMode: contain`) or any Lucide icon by name
(stroke colour default `#000`, size 24). With `onPress` it wraps itself in a
TouchableOpacity with role `imagebutton`.

**AutoImage** (`AutoImage.tsx`). Sizes a remote image to its aspect ratio
from `maxWidth` or `maxHeight`.

**EmptyState** (`EmptyState.tsx`). Ignite, unused (sad-face preset).

#### A3.4 Chat components

**Message bubble** (inline styles in `screens/ChatScreen.tsx` and
`screens/DirectMessageScreen.tsx`, verified).

```
row (gap 8, margin-bottom 16; row-reverse for own messages)
├─ User: avatar 40 round  [name label absolutely placed]
└─ bubble: flex 1, border 1 cyan900, square corners, padding-top 32
   ├─ time: absolute top 6, right 12, xs, cyan500
   ├─ Reply quote (if replying)
   └─ MessageContent
```

Others' bubbles fill `overlay20` (ChatScreen line 278); own bubbles fill
`overlay50` (line 286). `formatCreatedAt` prints `h:mm A` under 12 hours old
and `MMM DD` after.

**User** (`User.tsx`). Avatar 40, round, with an optional VenetianMask icon
(20, `cyan500`) for a blinded DM. The name label is absolute: left 61 (or
right 13 when reversed), top 8; bold `xs`, `cyan400`, max width 200. It
appends a reputation suffix from `social.getReputation`: "(loading)",
"(no reputation)" or a percentage with two decimals.

**MessageContent** (`MessageContent.tsx`, `utils/parser.tsx`). White text,
padding 12 horizontal, margin-bottom 12. Images (jpg, jpeg, gif, png, webp,
avif) render with `react-native-auto-height-image` at full width. Video
(mp4, webm, mov, ogv, avi, and mp3) renders in expo-av with native controls,
height 300. The first link is removed from the text and shown as a
LinkPreview. Later links, `nostr:` references and `#hashtags` are styled
`cyan500`.

**LinkPreview** (`LinkPreview.tsx`). Full-width image, then a 12-px padded
block: title bold `sm` white; description `sm` `cyan600`, three lines;
domain `cyan700`. Tap opens the link. States: "Loading..." and "Failed to
fetch open graph...". It uses `link-preview-js`.

**Reply** and **ReplyUser** (`Reply.tsx`, `ReplyUser.tsx`). A quote with a
3-px left border in `cyan500`, radius 1, padding 8 vertical and 2
horizontal, margin-left 12. The name is `sm` `cyan500`; one line of content
follows, or "Can't fetch event". Kind-4 content is decrypted for display.

**DirectMessageReply** (`DirectMessageReply.tsx`). The bar above the
composer while replying. Height 60, animated in by height and opacity.
Fill `overlay20`, top border `cyan900`. Contents: a Forward icon
(`cyan600`, 25), the sender name in a ReText (600, `cyan600`), the quoted
content in gray, and an X (TouchableFeedback, radius 20) that cancels.

**ChannelMessageForm** and **DirectMessageForm** (composer).

```
wrapper: top border 1 cyan900, fill overlay20, padding 8 vertical / 24 horizontal, gap 12, minHeight 40
├─ attach: 30-px transparent button, Paperclip 24 cyan700
├─ input pill: minHeight 40, radius 20, fill overlay20, padding 16 / 4, lineHeight 25, multiline
│    placeholder "Message" (or "Caption" with an attachment), colour cyan500
└─ send: 40-px round cyan600, ArrowUp 20 cyan100 — only when there is text or an attachment;
         otherwise a 5 × 40 spacer
attachment strip (when present): height 72, fill overlay20
├─ 50 × 50 thumbnail, radius 4
├─ "Attached image", cyan500
└─ X 16 cyan700 in a 40-px target; a cyan500 spinner while uploading
```

Image picking uploads to `https://nostrimg.com/api/upload` (verified).

**Faerie** (`components/Faerie/`). This is the AI chat persona: an avatar
image and message components. It is not an animation.

- `AIChannelDetail.tsx`: the home-list row for AI chat, with the faerie
  image and the name "AI Chat".
- `Message.tsx`: row padding 14 top, 20 sides, 18 bottom. Faerie rows fill
  `#1C1C1D`; user rows are transparent; the loading row is black and shows
  `typing-message.gif` at 54 × 38 with margin-top −16. Avatar 30, radius 15,
  black border. Text white 16, padding 20 left and 28 right.
- `MessageInput.tsx`: the same pill design as the composer, plus a mic
  toggle (40, transparent; `cyan600` while recording; press opacity 0.8)
  and a round `cyan600` send.
- `AnimatedTyping.tsx` (dead): one character per 1 ms, with a `|` cursor
  blinking every 250 ms in `#8EA960`.
- `CopyButton.tsx` (dead): Lucide Copy, 30, `#8E8E92`.
- `SolidScreen.tsx` (dead): black, padding 50 top and 30 bottom.

#### A3.5 List rows

**ChannelItem**, **DirectMessageItem** and **AIChannelDetail** share one
row layout (verified):

```
row
├─ avatar 50 round, border 0.6 #232324, margin-right 12, margin-top 2
└─ column
   ├─ heading row: name (bold, white, 1 line) ........ time (#7B7C7F)
   ├─ last author name (white)
   ├─ last message (#7B7C7F, max width 250)
   └─ divider 1 px #232324, margin 8 vertical
```

The unread badge (`#666` pill, minWidth 20, padding 3, `#000` text at 12)
is commented out in every live row; only the dead `ChannelDetail.tsx` keeps
it. The DM row appends " (you)" for your own key. Fallback names are "No
name" and the channel id. The placeholder avatar is a remote `void.cat` URL.

**ChannelManagerItem**. Avatar 44 on an `overlay20` disc; name bold; about
`sm` `rgba(255,255,255,0.5)`; padding 8 vertical.

**ContactItem**. Avatar 44 round; name bold (max width 200); shortened npub
(`npub1…` first 12 characters plus "...") in `xs`
`rgba(255,255,255,0.5)`; padding 8 vertical. Tap opens the User screen or a
DM.

**ContactPicker** (dead; Invite has its own copy). ContactItems in a
FlashList with a CheckCircle2 icon (16, `cyan500`).

**UserFeed** and **UserOffer**. Avatar 40; name `cyan500`; second line
`cyan700`. UserOffer hard-codes "Rep.: 90%" (a placeholder).

#### A3.6 Navigation components

**ScreenWithSidebar** (`ScreenWithSidebar.tsx`). Live (Home tab only).

- DrawerLayout 70 px wide, `slide` type, from the left (right in RTL).
  While open it dims the content with `overlay50`. Speed 2.
- Drawer content: fill `background`, a centred column below the top inset,
  gap 12. Two round 50-px `cyan400` buttons: Compass (to Channels) and
  LayoutList (to ChannelManager). Further buttons are commented out.
- Header: title, DrawerIconButton on the left, a white Plus (20, padding 16
  horizontal and 12 vertical) on the right that opens NewMessage.

#### A3.7 Trade components (unreachable)

The Listing screens are unreachable (A6), but their components exist.

**OfferForm** (`OfferForm.tsx`). A 45-px round `cyan400` FAB (Store icon)
at 24 from the bottom and right opens a BottomSheetModal with snap points
50%, 75% and 100% (starting at 75%), fill `#000`, border 1 `cyan500`. Title
"Create a offer" (bold `lg`; the grammar error is in the code). Fields are 44
high, border 1 `cyan500`, radius 8, padding 12, placeholder `cyan800`:
Price, Currency, Amount, Payment Methods, Expiration (default "1 hour"),
Geohash, Message. The button "Create offer" is `cyan500`, 44 high, radius 8,
pressed `cyan600`. The placeholders name fiat currencies and payment apps;
none of that carries over (Part B).

**ListingItem** (`ListingItem.tsx`). A box with fill `cyan950`, border 1
`cyan500`, radius 4, padding 12, minHeight 200. Rows of "Label:" (bold
`cyan400`) and value: Action, Item, Currency, Price, Amount, Min. amount,
Expiration, Payment. It reads the `x=listing` tag and a JSON `data` field.

**ListingOfferItem**. Rows: Message, Currency, Price, Amount, Expiration,
Payment.

#### A3.8 Other components

| Component | Status | Note |
| --- | --- | --- |
| `CityBackground.tsx` | Live (logged-out Home only) | Absolute fill of `assets/images/city-cyan.png`, `cover`, under a black view at 0.75 opacity (zIndex 1000) |
| `RelayProvider.tsx` | Live | React context only; no UI |
| `TextWithImage.tsx` | Dead | |
| `ChannelDetail.tsx` | Dead | Keeps the unread badge |
| `Faerie/AnimatedTyping`, `CopyButton`, `SolidScreen` | Dead | See A3.4 |
| `EmptyState.tsx` | Dead (Ignite) | |

### A4. Icons and images

#### A4.1 Icon sources (verified)

| Source | Files | Use |
| --- | --- | --- |
| Lucide (`lucide-react-native` 0.214) | npm | Almost every icon on live screens |
| SVG icons | `app/components/icons/*.svg`, 24 px, filled outline, `currentColor` | `chat`, `profile` and `settings` drive the tab bar. `chats`, `contacts` and `userplus` are unused |
| PNG registry | `assets/icons/*.png` at @1x, @2x and @3x | Ignite set: back, bell, caretLeft, caretRight, check, clap, community, components, debug, github, heart, hidden, ladybug, lock, menu, more, pin, podcast, settings, slack, view, x. Live screens use `back` (header) and `ladybug` (error screen) |

Lucide icons used outside the Demo (verified by searching imports and
`leftIcon` / `rightIcon` / `icon` props): ArrowRight, ArrowUp, Axe (the
trailing icon on the Profile popup's menu items), Bell, Boxes, Bot,
CheckCircle2, ClipboardCopy (Backup), Compass, Edit (Profile's Account
section), Eye, EyeOff, FolderLock, Forward, Github, Globe, HelpCircle (User
screen help), ImagePlus, LayoutList, List, LogOut, Mic, Paperclip, Plus,
PlusSquare, Shield, Store and User2 (both unreachable), Twitter, UserMinus,
UserPlus, Users, Users2, VenetianMask and X. Copy appears only in the dead
CopyButton. The header back icon (`back`) and the CreateChannel submit
(`check`) come from the PNG registry.

#### A4.2 Images (`assets/images/`, sizes measured)

| File | Size | Status | Content |
| --- | --- | --- | --- |
| `city-cyan.png` | 1248 × 960 | Live (Home) | A neon cyberpunk street in cyan duotone on black |
| `city.png` | 1248 × 960 | Unused | The same street in full colour |
| `faerie.png` | 1024 × 1024 | Live (AI avatar) | A neon teal line-drawing fairy on dark |
| `face.png` | 117 × 117 | Live | A blue-gray user silhouette; the user avatar in AI chat |
| `typing-message.gif` | 498 × 286 | Live | The AI loading indicator |
| `logo-channel.png` | 1014 × 932 | Unused | Cyan Arcade logo |
| `lobby.png` | 1024 × 1024 | Unused | Blue sci-fi lounge |
| `handshake.png`, `handshake2.png` | 1024², 634² | Unused | Robot handshake |
| `sad-face*.png` | 147 × 167 (@1x) | Ignite | EmptyState |
| `logo*.png`, `welcome-face*.png`, `rnr-*`, `rnl-*`, `rnn-*`, `cr-logo*` | various | Ignite | Demo leftovers |
| `app-icon-*.png` | 1024 | Ignite | Not the real icon |
| `splash-logo-*.png` | various | Live | The Arcade logo on a black square |

The real iOS icon is `ios/arcade/Images.xcassets/AppIcon.appiconset`: a cyan
(about `#00C8E0`, inferred from the pixels) rounded-triangle "A" outline
with a gap at the bottom centre and a keyhole shape inside, with an outer
glow, on black. The boot splash (`ios/arcade/BootSplash.storyboard`) is the
same logo at 212 × 212 without the glow, on black.

`assets/top.json` (104 KB) is a bundled snapshot of popular kind-40
channels; the app also fetches a live copy from the `arcade-static` GitHub
repository (raw). Audit 2 covers its data.

### A5. Screen layouts

Phone screens, top to bottom. "Header" means A3.3 Header (56 high, back icon
`cyan400`). Copy is verbatim. Screen files are in `app/screens/`.

**Home** (logged out, `HomeScreen.tsx`)

```
CityBackground (city-cyan.png, 75% black over it)
├─ "arcaDE"  (Protomolecule 75/100, letter-spacing 4, white, #00ffff shadow 4), margin-top 55%
└─ bottom, full width:
   ├─ [Create Account]  ghost: border cyan500, fill rgba(0,0,0,0.5)
   └─ [Login]           ghost: border cyan900
```

**Login** (`LoginScreen.tsx`)

```
Header (back only)
"Enter access key"            subheading at xl
[ nsec or hex private key  (eye) ]   Arcade field, secure, with an eye toggle
[Enter]                        primary
progress text: "Fetch user's metadata" → "Found N joinded channels. Rejoining..."
               → "Fetching private messages.." → "Loading..."
```

**CreateAccount** (`CreateAccountScreen.tsx`)

```
Header "Create account"
avatar 80 with ImagePlus over an overlay80 scrim (tap picks an image)
[ Username * ]  with a trailing "@arcade.chat" label
[ Display Name ]
[ About ]
[Continue]  primary
```

**Main tabs** (TabNavigator, A6): Contacts, Home, Profile.

**HomeMessages** ("Home", `HomeMessagesScreen.tsx`)

```
ScreenWithSidebar header: (hamburger)  Home  (+ → NewMessage)
FlashList, newest last-message first, pull to refresh:
  ChannelItem / DirectMessageItem / AIChannelDetail rows, each fading in (100 ms × index, 800 ms)
empty: "No data..."
```

It merges joined channels, kind-4 DMs and AI conversations (kind
10101010; see audit 2 for the kinds).

**Contacts** (`ContactsScreen.tsx`)

```
Header "Contacts"  (+ cyan400 → AddContact)
ContactItem rows, each with trailing icons (20, cyan100):
  Globe (only for legacy-DM contacts) and UserMinus (remove)
empty (gray):
  "You have no contacts on Arcade yet"
  "- Invite friends to try Arcade"
  "- Search people by public key or npub"
```

**Profile** (tab, `ProfileScreen.tsx`)

```
banner image, height 200 (default: a void.cat URL)
avatar 80, margin-top -40
name (bold lg cyan400)
npub shortened (sm cyan600), tap copies → "Copied" for 500 ms
section box (border cyan900, radius 4) "Account"  [edit icon → BlurredPopup]
  Username / NIP-05 / About
section box "Settings"
  ListItem Manage Relays (Boxes)
  ListItem Notifications (Bell)   — hidden on iOS
  ListItem Privacy (EyeOff)
  ListItem Backup (Shield)
section box "Resources"
  ListItem Changelog (List) / Twitter / GitHub
[Logout]
```

**User** (someone else's profile, `UserScreen.tsx`)

```
Header "Profile"
banner 150; avatar 80, border 2 black, margin-top -40
name / nip05 / about
[Message]  [Follow | Unfollow]  [Private follow | Stop private follow]
"Nobody can see your private follow"
"Contact settings"
  switch "Use legacy, unblinded DM's"
  switch "Hide this contact (private follow)"
```

**EditProfile** (`EditProfileScreen.tsx`)

```
Header "Edit profile"
avatar picker
[ Display Name ] [ Username ] [ NIP-05 ] [ About ]
[Update]  primary, cyan500, height 50
```

**Chat** (channel, `ChatScreen.tsx`)

```
Header: back · channel name (first 20 chars + "...") · Users (private only → ChannelMembers) · LogOut (leave)
inverted FlashList of message bubbles (A3.4), each in a SwipeableItem
DirectMessageReply bar (while replying)
ChannelMessageForm
safe-area fill overlay20; keyboardOffset 104
empty: "No messages", or a spinner while loading
```

**DirectMessage** (`DirectMessageScreen.tsx`). The same layout. Title is the
contact's name or "Direct Message". A VenetianMask marks a blinded DM.

**NewMessage** (`NewMessageScreen.tsx`)

```
Header "New Chat"
box (border cyan900):
  ListItem "New channel" (Users2)
  ListItem "New private channel" (FolderLock)
  ListItem "New AI chat" (Bot)
"Recent contacts" (md bold cyan600)
ContactItem rows → DM
```

**CreateChannel** (`CreateChannelScreen.tsx`)

```
Header "New Channel" | "New Private Channel"   (check on the right submits)
avatar picker 60
[ Channel name ]   2-px bottom border cyan900
[ Description ]    helper "You can provide an optional description for your channel"
toggle "Private channel"
```

**Channels** (from the drawer, `ChannelsScreen.tsx`)

```
Header "Channels"  (+)
Card tiles (border cyan500 or cyan100, radius 6, fill overlay20):
  image 60 radius 4 · name (bold) · about
  footer bar (top border cyan600, height 45): "Join" | "Joined"
loading: Skia ActivityIndicator
```

**ChannelManager** (`ChannelManagerScreen.tsx`). Header "Channel Manager".
ChannelManagerItem rows with UserPlus (invite) and UserMinus (leave). Empty:
"No channels".

**ChannelMembers** (`ChannelMembersScreen.tsx`). Header "Members" with a
Plus to Invite. Empty: "Private channel has no members", "- Click Plus
button above to invite".

**Invite** (`InviteScreen.tsx`). Header "Invite" with a Plus that opens a
bottom sheet ("Invite by pubkey", field "Public key or npub", placeholder
"npub...", [Add]). Selectable contacts show CheckCircle2 or "Added". A
50-px round `cyan500` FAB with ArrowRight confirms.

**AddContact** (`AddContactScreen.tsx`). Header "Add contact" with a Plus
that opens a sheet (snap points 36% and 50%; "Add contact", "Public key or
npub", [Add]). "Suggestions" lists trending profiles from nostr.band with
[Add] / [Remove] pills (36 high, border `cyan500`, radius 8, fill
`overlay20`).

**RelayManager** (`RelayManagerScreen.tsx`). Header "Relay Manager" with a
Plus that opens a sheet ("Add custom relay", "Relay URL", placeholder
"wss://", [Add]). A SectionList: "Connected" (note: "You always need at
least one relay to use Arcade") and "Suggested relays" (from
api.nostr.watch). Rows have border `cyan900`, radius 4, and an X or a
PlusSquare.

**NotificationSetting** (`NotificationSettingScreen.tsx`). Header
"Notifications". "Notifications for chats" with switches "Private Chats" and
"Channels". [Update].

**PrivacySetting** (`PrivacySettingScreen.tsx`). Header "Privacy".
"Blinded (unleak metadata)" with a switch "Direct message". Note: "Currently,
this feature only supported by Arcade and not it compatible with other
clients." [Update].

**Backup** (`BackupScreen.tsx`). Header "Backup". Read-only secure fields
"Private Key (Hex)" and "Private Key (Nsec)", each with a Copy / Copied
button. "Safety Tips": "Your account is secured by a secret key. This key
gives full access to your account, never share it to anyone."

**Changelog** (`ChangelogScreen.tsx`). Header with a GitHub link to the
releases page. Entries from v0.3.1 (12 Jul 2023) back to v0.1.0 "Oslo"
(12 June 2023), split by `cyan800` dividers with 24 vertical margin.

**AIChannel** (`screens/AI/AIChannel.tsx`). Header "AI Chat". An inverted
list of Faerie messages and the Faerie MessageInput. It posts to
`https://api.arcade.chat/message`.

**ErrorDetails** (`screens/ErrorScreen/`, Ignite). Ladybug icon 64;
"Something went wrong!" in `error`; a subtitle; the component stack in a
`neutral800` box (radius 6); a "RESET APP" button in `error`.

Unreachable screens (registered but with no route that leads to them):

- **TrainAI** (`screens/AI/TrainAI.tsx`): "AI Settings", a "MODEL" section
  with iOS-settings-style cards for GPT-3.5, GPT-4 and Claude, each with a
  subtitle and a price.
- **Discover**: a single "Channels" button.
- **Listing** and **ListingDetail**: Card lists of ListingItem, then
  "Offers" with UserOffer, ListingOfferItem and [Accept]. A hard-coded
  group id.
- **Demos** (Ignite DemoScreen).

### A6. Navigation chrome

**AppNavigator** (`navigators/AppNavigator.tsx`, verified). A native stack
with `headerShown: false`; each screen sets its own Header through
`setOptions` or `useHeader`. Animation `slide_from_right`. The
NavigationContainer picks `DarkTheme` or `DefaultTheme` from the system
scheme, but every colour is hard-coded dark, so the result is dark in both
(inferred: the light theme only changes React Navigation's own defaults,
which Arcade hides). BlurredPopupProvider wraps the stack. Logged out, the
stack shows the Auth navigator (`AuthNavigator.tsx`: Home, Login,
CreateAccount).

**TabNavigator** (`navigators/TabNavigator.tsx`).

| Property | Value |
| --- | --- |
| Tabs | Contacts (`profile.svg`), Home (`chat.svg`, initial), Profile (`settings.svg`) |
| Labels | None |
| Icons | 30 px; inactive `cyan800` (line 35), active `tint` |
| Bar | Fill `background`; top border 1 `cyan800` (line 49) |
| Height | Bottom inset + 40 (Android + 60) |
| Item | Padding-top 16 |
| Keyboard | `tabBarHideOnKeyboard` |

**TabBar** (`navigators/TabBar.tsx`). Dead. The Frame-based HUD tab bar:
absolute at bottom 30 (iOS) / 20, inset 10 left and right, a Frame 65 high
with `internalSquareSize` 10, tabs fading in at 500 + 50 × index ms over
300 ms.

**HudNavigator** (`navigators/HudNavigator.tsx`) and the Skia SplashScreen
(`screens/SplashScreen/`) are dead.

**Drawer**: ScreenWithSidebar (A3.6), on Home only.

**Sheets**: `@gorhom/bottom-sheet` BottomSheetModal for Invite, AddContact,
RelayManager and OfferForm. Fill black, border 1 `cyan500`.

**Alerts**: React Native `Alert.alert` for every confirmation and error.

### A7. Copy and i18n

`app/i18n/en.ts` holds only Ignite strings (verified):

| Key | Text |
| --- | --- |
| `common.ok`, `cancel`, `back` | "OK!", "Cancel", "Back" |
| `errorScreen.title` | "Something went wrong!" |
| `errorScreen.friendlySubtitle` | "Uh-oh, there was en error. Please tell us about it and we'll fix it! In the meantime you can try refreshing your app below." |
| `errorScreen.reset` | "RESET APP" |
| `emptyStateComponent.generic` | "So empty... so sad", "No data found yet. Try clicking the button to refresh or reload the app.", "Let's try this again" |
| `welcomeScreen.*` | Ignite welcome text (unused) |

`ar.ts` and `ko.ts` translate the same Ignite keys. All product copy is
hard-coded in the screens. A5 gives each screen's visible copy. The alerts
and errors, by screen, are:

| Screen | Alerts and errors (verbatim) |
| --- | --- |
| Login | "Access key as nsec or hex private key is required"; "Invalid key. Did you copy it correctly?" |
| CreateAccount | "Username is required"; "Username is invalid, please check again" |
| Contacts | "Legacy direct message" / "You're using legacy direct message for this contact, all messages are encrypted, but everyone can see you interact with this contact"; "Confirm to remove this contact" / "Are you sure?" / "Cancel" / "Confirm" |
| User | "Blinded DM automatically enabled when you use private follow"; "Cannot save to network: …" |
| EditProfile | "Failed to save settings: …" |
| Chat | "Confirm leave channel" |
| CreateChannel | "Channel name is required"; "Error, please check information again: …" |
| Invite | "You have not invited anyone yet." / "Are you sure you want to skip this step?"; "Confirm choosing those selected contacts"; "User has been added"; "Invalid user, please make sure you enter valid public key or npub" |
| AddContact | "Can't fetch trending profiles, service temporarily unavailable"; "Invalid contact, please make sure you enter valid public key or npub"; "Contact has been added already" |
| RelayManager | "Confirm to add this relay"; "You have add this relay"; "Confirm remove this relay"; "You're using this relay, please add another"; "Relay must use websocket protocol (start with wss:// or ws://), please check again"; "Relay is not valid, please check again" |
| PrivacySetting | "Failed to save settings: …. Please try again later" |
| AI chat | "Couldn't find your user ID - try reopening the app"; "Message too short" / "What is that, a message for ants?"; placeholders "Write your message here" and "Ask your question here" |
| Backup | "Copied" |

Several strings have spelling or grammar errors ("joinded", "en error",
"You have add", "not it compatible", "Create a offer", "DM's"). Part B fixes
them in the port rather than copying them.

---

## Part B: the adaptation to OpenAgents iOS

### B1. What we have

**The kit and skin.** `components/spec.md` defines five layers: core,
layout, paint, skin and backend. Application code builds a keyed tree with
`k_*` builders from `components/kit/` (`frames.coax`, `listing.coax`,
`settings.coax`, `dock.coax`, `work.coax`, `stage.coax`). The skin decides
how each role looks. The mono skin (`components/skin/mono.coax`) is black,
white and grays, with ARWES frames that the browser backend draws as SVG.
The `components-check` gate forbids colour literals and sizes inside `kit/`.

Mono tokens (`mono.coax`, CSS tokens):

| Token | Value |
| --- | --- |
| `--ground` | `#000` |
| `--surface` | `#0a0a0a` |
| `--raised` | `#161616` |
| `--line-dim` | `#2a2a2a` |
| `--line` | `#6b6b6b` |
| `--line-bright` | `#fff` |
| `--ink` | `#f2f2f2` |
| `--ink-muted` | `#9a9a9a` |
| `--ink-faint` | `#5c5c5c` |
| `--sans` / `--mono` | Geist / Geist Mono |

Mono frame roles and their ARWES kinds: button (octagon), panel (corners),
field (underline), notice (lines), header (header), row (octagon), bubble
(corners), stage (nefrex), card, mark and box (octagon). The skin's settings
are: button square 8; panel corner length 12, width 1; bubble corner 8 and
width 1; row square 6; card 10. Motion: enter 0.55 s, stagger 0.035 s, text
70 characters a second, "Copied" 1.6 s. The body is 14/1.6 on an ARWES grid
of 40 px (`#070707` lines, `#181818` dots). The primary button fills
`#c8c8c8` with black text. A disabled button has a dashed line. Busy rows and
running glyphs turn (2.4 s and 1.8 s). Every motion has a reduced-motion
fallback. Under a coarse pointer, every control is at least 44 px.

The ARWES ports in `components/core/arwes/` (`frames.coax`, `box.coax`,
`motion.coax`, `fx.coax`) compute frames as point lists in screen points.
They use no DOM or SVG (`frames.coax` header: "All dimensions are resolved
screen points. No SVG/DOM"). A native host can therefore stroke the same
frames with `CAShapeLayer` paths. No native host does this yet.

**OpenAgents iOS today** (`apps/openagents-ios/`, Coax Native). The app runs
compiled Coax in JavaScriptCore and projects the same kit tree to UIKit.
`apps/shared/native/NativeProgram.swift` walks the tree and turns it into a
flat list of roles. `NativeControls.swift` renders that list:

| Role | UIKit control | Notes |
| --- | --- | --- |
| `heading` | UILabel, `.largeTitle` or `.title2` | |
| `text` | UILabel, `.body` | |
| `notice` | UILabel, `.footnote`, `.secondaryLabel` | |
| `button` | UIButton `.bordered()`; class `lk` gives `.plain()` | Min height 44 |
| `field` | UITextField `.roundedRect`; `multiline` gives UITextView (radius 8) | Keyboard and content types from the element |
| `toolbar` | A 44-px header row of plain buttons and a label | Status button states loading, ready, offline, idle |
| `composer` | `NativeComposer`: UITextView on glass (`UIGlassEffect` on iOS 26, else dark blur), radius 27, DM Sans 16/23 | Geometry in `components/layout/native-composer.coax` |
| `message` | `NativeMessage`: TextKit rich text, code and table views | User bubble right, 85% wide, radius 20; assistant prose unframed |
| `tool` | `NativeTool`: 28-pt work rows with SF Symbols | `nt_work()` |
| `transcript` | A scroll that follows the end | |

Everything sits in one vertical `UIStackView` inside a scroll view, with the
composer docked to the keyboard layout guide. There are no native images,
avatars, lists of rows, switches, tabs, sheets or drawn frames yet.

The native colours come from `components/skin/native-chat.coax`
(`nt_skin()`): ink `#e5e5e5`, strong `#f5f5f5`, muted `#999999`, code fill
`#191919`, bubble `#262626`, border `#383838`, link `#8cb4ff`. Body 16/23,
radius 20. Fonts are DM Sans (`components/fonts/`, OFL). Three colours in
the current app are not grayscale: the send disc `#346bf1`
(`NativeComposer.swift` line 91), the link colour `#8cb4ff`, and the
refresh control's system green and orange (`NativeControls.swift` line 70).

**Web `/chat`** (`apps/openagents-web/chat-web.coax`). Built from the same
kit on the mono skin: `k_side` for the 300-px side, `k_row` and `k_fold`
for the list, `k_user` for the person's message (dim frame, right, at most
72% wide), `k_agent_text` for unframed prose, `k_composer_begin/end` at the
foot, `k_select_*` for the model menu, `k_notice` and `k_card`. The owner
rule for new UI is to use the kit on the mono skin and look like `/chat`.

**Icons.** `components/core/icons.coax` holds the Apps SDK UI set (758
names, `IC_*`), shown with `k_icon` and `k_icon_button`. The native
projection uses SF Symbols in `NativeComposer`, `NativeMessage` and
`NativeTool`.

### B2. Grayscale translation

#### B2.1 How meaning survives without hue

Arcade uses five cyan steps to say five things: structure (900), quiet
accent (700–800), accent (500–600), emphasis (400) and action fill
(500–600). Without hue, the port carries the same meaning with lightness,
line weight, fill and shape:

- **Lightness steps** replace the cyan ramp, one gray step per cyan step.
- **Filled versus outlined** separates a primary action (white fill, black
  text) from a secondary one (outline).
- **Frames** separate a framed thing (the person's message, a field, a
  card) from unframed prose, as in `/chat`.
- **Words and icons** carry state that Arcade showed only in colour. Errors
  get the error notice (a bright frame with words), not red. A blinded DM
  keeps its mask icon. Live, sent and failed states get words.
- **Translucent cyan washes** become flat dark grays. On black, an 11% cyan
  wash has a luminance near `#0e1a1e`, which `--surface` and `--raised`
  approximate.

#### B2.2 Token table

The mono column is the web token; the native column is the `nt_skin()` role
or the value to add to it. Values are the targets the port uses.

| Arcade token | Value | Meaning | Mono token | Native (`nt_skin`) |
| --- | --- | --- | --- | --- |
| `almostBlack` / `background` | `#030303` | Screen | `--ground` `#000` | System black (dark appearance) |
| `black` | `#000` | Outer view, sheets | `--ground` | Same |
| `overlay50` | cyan 5% | Own bubble, quiet wash | `--surface` `#0a0a0a` | New role `surface` `0x0a0a0aff` |
| `overlay20` | cyan 11% | Others' bubble, composer, field, card fill | `--raised` `#161616` | `code_bg` `#191919` (closest existing) |
| `overlay80` | black 50% | Scrim | `rgba(0,0,0,.5)` scrim | Same scrim |
| `cyan950` | `#083344` | Swipe disc, listing fill | `--raised` | `bubble` `#262626` |
| `cyan900` / `separator` | `#164e63` | Hairlines, frames | `--line-dim` `#2a2a2a` | `border` `#383838` |
| `#232324` | | Row dividers, avatar ring | `--line-dim` | `border` |
| `cyan800` | `#155e75` | Inactive tab icon, tab border, placeholder | `--ink-faint` `#5c5c5c` | New role `faint` `0x5c5c5cff` |
| `cyan700` | `#0e7490` | Quiet icons, domains | `--ink-faint` | `faint` |
| `cyan600` | `#0891b2` | Send disc, npub, secondary text | `--line` `#6b6b6b` for lines; `--ink-muted` for text | `muted` `#999999` |
| `cyan500` | `#06b6d4` | Timestamps, links, placeholders, accent borders | `--ink-muted` `#9a9a9a`; `--line` for borders | `muted`; links use `strong` plus underline |
| `cyan400` | `#22d3ee` | Names, back icon, emphasis | `--ink` `#f2f2f2` | `strong` `#f5f5f5` |
| `tint` | `#5BC6E0` | Active tab, header actions | `--line-bright` `#fff` | White |
| `cyan100` | `#cffafe` | Glyph on a filled button, private border | `--ink` | `strong` |
| `text` / `neutral100` / `white` | `#fff` | Body text | `--ink` | `ink` `#e5e5e5` |
| `gray` / `#7B7C7F` | | Row time, last message | `--ink-muted` | `muted` |
| `rgba(255,255,255,.5)` | | Secondary text in rows | `--ink-muted` | `muted` |
| `textDim` | `#564E4A` | Disabled text | `--ink-faint` | `faint` |
| Primary fill `cyan500` | | Primary button | `.btn` primary `#c8c8c8`, black text | White fill, black text |
| Round send `cyan600` | | Send | White disc, black arrow | Replace `#346bf1` with white disc, black arrow |
| `#1C1C1D` | | AI message row | None: AI prose is unframed | None |
| `error` / `angry500` | `#C03403` | Error text and button | `k_notice` (bright frame) and words | `notice` role plus a leading symbol |
| `errorBackground` | `#F2D6CD` | | None | None |
| `#00ffff` text shadow | | Wordmark glow | Dropped | Dropped |
| Popup fill `rgba(255,255,255,.75)` | | Long-press menu | `.menu` | System `UIMenu` |
| Press `rgba(255,255,255,.1)` | | Press feedback | `:active` on `--raised` | UIKit highlight |

Two new native roles are needed: `surface` (`#0a0a0a`) and `faint`
(`#5c5c5c`). They mirror mono tokens that `nt_skin()` lacks today.

#### B2.3 Signature elements

| Element | Decision | Reason |
| --- | --- | --- |
| Corner-bracket Frame | Keep, in gray, through the skin's ARWES `corners` kind | It is Arcade's HUD identity and matches mono's panel and bubble frames. Arcade's own frame is a box with four square corner markers; ARWES `corners` (and `octagon` for buttons) gives the same read. Drawn natively from `af_frame` point lists |
| Frame enter animation (scale in, 200 ms) | Keep, as the skin's enter motion | Mono already assembles frames over 0.55 s with a stagger and honours reduced motion |
| Frame press highlight (0.2 → 0.7 fill) | Simplify to a fill step (`--surface` to `--raised`) | Same meaning, one step, no opacity animation to tune |
| Glow (wordmark shadow, glowing spinner, BackgroundGradient, Spotlight) | Drop | Glow is a hue effect; in gray it reads as blur. Three of the four are dead code already |
| City background | Drop from the app. If the owner wants a title screen, keep a grayscale duotone of `city-cyan.png` behind the login screen only, dimmed 75% | The mono ARWES grid already gives the backdrop. The image is a 1248 × 960 bitmap with no use beyond one screen |
| Faerie (AI persona) | Drop the fairy image. The AI uses the OpenAgents mark (the power symbol, `/mark.svg#i`) as its avatar, and its replies are unframed prose like `/chat` | The fairy is a cyan illustration that does not survive grayscale. OpenAgents already has one agent identity and voice |
| Typing GIF | Replace with the mono waiting caret (`.msg-ai[data-state=waiting]`) on web and the native spinner already in `NativeMessage` | A GIF cannot follow the skin |
| Swipe-to-reply with donut | Keep, in gray (white donut at 50%, `--raised` disc) | It is good interaction and has no colour dependency |
| Long-press blur menu | Simplify to the system context menu (`UIContextMenuInteraction`) | UIKit gives the blur, the lifted preview and the haptic for free, with accessibility |
| Translucent cyan glass | Become flat grays (B2.2) | Hue-only effect |
| Square bubbles with a timestamp corner | Keep the frame idea through mono's bubble frame (corners kind) | Matches `/chat`'s framed person message; see B5 for the native bubble conflict |
| Space Grotesk / Protomolecule | Replace (B4) | |
| App icon (cyan triangle A) | Do not port | OpenAgents iOS keeps its own icon and the power-symbol mark |

### B3. Component mapping

Category: **E** = existing kit component, **X** = extend an existing kit
component, **N** = new kit component. Native: **UIKit** = a plain control
the projection has or can add with no custom drawing; **Draw** = needs
custom drawing (CAShapeLayer or Core Animation) in the native host;
**New view** = a new native view type in `apps/shared/native/`.

| Arcade component | Kit target | Cat. | Native today | Native work |
| --- | --- | --- | --- | --- |
| Text presets | `k_text` with tags; skin type scale | E | UILabel | Map presets to skin sizes (B4) |
| Button primary | `k_button` (primary state) | E | UIButton `.bordered()` | Restyle: white fill, black text (skin), or Draw octagon frame |
| Button ghost | `k_button` (default state) | E | UIButton `.bordered()` | Draw octagon frame in `--line` |
| Text link button | `k_link` / `k_quiet` | E | UIButton `.plain()` (`lk`) | None |
| Round icon button (40) | `k_icon_button` | E | Not projected | UIKit: UIButton with an image; add the `icon` role |
| FAB | `k_icon_button` plus a skin class | X | Not projected | UIKit button, pinned to the safe area |
| TextField / Arcade field | `k_field`, `k_input`, `k_secret` | E | UITextField | Field underline frame: Draw |
| Eye toggle on a secret | `k_secret` reveal | X | Not projected | UIKit: `isSecureTextEntry` plus a right-view button |
| Toggle (switch) | `k_switch`, `k_switch_row` | E | Not projected | UIKit: `UISwitch`, tinted white |
| Toggle (checkbox, radio) | `k_choice` / `k_choices` | E | Not projected | UIKit: SF Symbol check in a button |
| ListItem | `k_row` / `k_place` (icon + title) | E | Not projected | UIKit: a row view (stack + label + symbol); new `row` role |
| Section box (Profile) | `k_set_group` / `k_set_row` | E | Not projected | UIKit inset-grouped look, framed by the skin: Draw for the corners |
| Card / channel tile | `k_card` or `k_entry` (image, title, line, facts) | E | Not projected | New view with an image; Draw frame |
| Header | `k_header` | E | `toolbar` row | Extend toolbar with back, title and trailing icons; or use `UINavigationBar` |
| Screen | Page tree plus the native scroll | E | NativeControls scroll | None |
| Message bubble (others) | `k_message` assistant=false, from another author | X | NativeMessage (user right) | Extend: author name, avatar, time, left alignment for others |
| Message bubble (own) | `k_user` / `k_message` | E | NativeMessage | Add time |
| User (avatar + name) | `k_who` plus a new avatar | X | Not projected | New view: round image, 40 |
| Avatar | New `k_avatar` (image, initials fallback, size) | N | None | New view: `UIImageView` in a circle, cached loading, gray ring |
| MessageContent: text | `k_inline` / `k_agent_text` | E | NativeRichText | None |
| MessageContent: image | `k_image` | E | Not projected | New view: image with aspect fit, loading state |
| MessageContent: video | New `k_video` | N | None | UIKit: `AVPlayerViewController` |
| LinkPreview | New `k_preview` (image, title, description, domain) | N | None | New view; fetch is a service, not UI |
| Reply quote | `k_inline` with quote marks (mono quote stripe) | X | NativeRichText has `quote` | Reuse the quote stripe |
| Reply bar above composer | `k_composer_begin` plus a reply slot | X | NativeComposer | Extend composer with a dismissible quote row |
| Composer | `k_composer_begin/end` | E | NativeComposer | Attach button needs enabling; send disc goes white |
| Attachment strip | `k_file` / `k_drop` inside the composer | X | Not projected | Extend NativeComposer with a thumbnail row |
| SwipeableItem | New `k_swipe` behaviour on a message | N | None | Draw: pan gesture, donut `CAShapeLayer` with `strokeEnd` |
| AnimatedDonut | Part of `k_swipe`; or `k_meter` for a static ring | X | None | Draw |
| ActivityIndicator | `k_glyph` (run state) / `k_loading` | E | `UIActivityIndicatorView` | None (system spinner); or Draw the mono turning ring |
| BlurredPopup | `k_menu` | E | Not projected | UIKit: `UIContextMenuInteraction` |
| Bottom sheet | `k_pane` (dock) | X | Editor sheet exists for worlds | UIKit: `UISheetPresentationController` with detents 36/50/75% |
| Alert / confirm | New `k_confirm` (title, body, two actions) | N | None | UIKit: `UIAlertController` |
| ScreenWithSidebar / drawer | `k_side` | E | None | Replace the drawer with a tab or a navigation push (B5) |
| DrawerIconButton | Drop | | | |
| Tab bar | New `k_tabs` (three places) | N | None | UIKit: `UITabBarController`, SF Symbols, white active, `--ink-faint` inactive |
| Channel / DM row | `k_entry` or `k_row` with avatar, time, last line | X | None | New row view with avatar |
| ContactItem | `k_row` with avatar and trailing actions (`k_row_with`) | X | None | Same row view |
| Empty state | `k_notice` (kind "empty") | E | `notice` label | None |
| Copy-to-clipboard | `k_code_copy` pattern / clipboard service | E | Clipboard service exists | None |
| Profile banner | `k_image` (big) | E | None | Image view, 200 high |
| Changelog | `k_document` or Markdown via `k_agent` | E | NativeMessage | None |
| OfferForm / ListingItem | `k_card` + `k_fact` rows | E | | Only if trading returns; Bitcoin and Lightning only |
| CityBackground | Drop (or one login image) | | | |
| Faerie components | `k_message` assistant=true | E | NativeMessage | None |
| Frame (drawn) | Skin frames via `k_frame` | E | None | Draw: `af_frame` → `CAShapeLayer` (one shared view) |

The largest single native addition is **one framed-view primitive**: a
`UIView` that takes a frame kind and its settings from the skin, calls the
Coax `af_frame` export for its bounds, and strokes the result with
`CAShapeLayer`s (background, line and decoration layers, matching
`AfPoint.layer` 0, 1 and 2). Buttons, fields, cards, section boxes and
bubbles then share it. Without it, the port looks like stock iOS controls
in gray, not like Arcade or `/chat`.

### B4. Typography and icons

**Typography.**

- Replace Space Grotesk and Protomolecule. Protomolecule is a display face
  with no role in OpenAgents. Space Grotesk would add a third family.
- The current native app uses DM Sans (body 16/23), from the T3 composer
  work. The web `/chat` uses Geist and Geist Mono. Pick one for native (B5,
  decision 1). The recommendation is to keep DM Sans for native body text,
  because it is already shipped, licensed and measured, and to add Geist
  Mono only for labels and meta text, where mono uses uppercase mono with
  letter spacing.
- Map Arcade's scale onto the native scale:

| Arcade | Native target |
| --- | --- |
| `xxl` 36/44 heading | `heading` h1 (`.largeTitle`) |
| `xl` 24/34, `lg` 20/32 | `heading` h2 (`.title2`) |
| `md` 18/26 (header titles) | 16 semibold (toolbar title, as today) |
| `sm` 16/24 (body) | DM Sans 16/23 (`nt_skin().body`, `line`) |
| `xs` 14/21 (names, times) | 14, `muted` |
| `xxs` 12/18 | 12 mono uppercase meta (`.side-m` style) |

- Keep Dynamic Type through `UIFontMetrics`, as `NativeTypography` already
  does. Arcade had none.

**Icons.**

- Replace Lucide, the Arcade SVGs and the Ignite PNGs.
- On web, use Apps SDK icons by name through `k_icon`.
- On native, the projection uses SF Symbols today. Map each Arcade icon to
  both:

| Arcade (Lucide) | Apps SDK (`IC_*`) | SF Symbol |
| --- | --- | --- |
| ArrowUp (send) | `IC_ARROW_UP` | `arrow.up` |
| ArrowRight | `IC_ARROW_RIGHT` | `arrow.right` |
| Paperclip | `IC_PAPERCLIP` | `paperclip` |
| Plus / PlusSquare | `IC_PLUS` / `IC_PLUS_SQUARE_ADD` | `plus` / `plus.square` |
| X | `IC_X` | `xmark` |
| Check / CheckCircle2 | `IC_CHECK` / `IC_CHECK_CIRCLE` | `checkmark` / `checkmark.circle` |
| ClipboardCopy | `IC_COPY` | `doc.on.doc` |
| Edit | `IC_EDIT_PENCIL` | `pencil` |
| HelpCircle | `IC_QUESTION_MARK_CIRCLE` | `questionmark.circle` |
| Axe (menu decoration) | Drop | Drop |
| back (PNG) | `IC_CHEVRON_LEFT` | `chevron.left` |
| Eye / EyeOff | `IC_EYE` / `IC_EYE_OFF` | `eye` / `eye.slash` |
| Forward (reply) | `IC_REPLY` | `arrowshape.turn.up.left` |
| Users / Users2 | `IC_USERS` / `IC_GROUP` | `person.2` |
| UserPlus | `IC_USER_ADD` | `person.badge.plus` |
| UserMinus | `IC_USER` plus `IC_MINUS` (no single icon) | `person.badge.minus` |
| Globe | `IC_GLOBE` | `globe` |
| Bell | `IC_BELL` | `bell` |
| Shield | `IC_SHIELD_LOCK` | `lock.shield` |
| Boxes (relays) | `IC_NODES` | `server.rack` |
| List | `IC_DOCUMENT` | `list.bullet` |
| Compass | `IC_COMPASS` | `safari` |
| LayoutList | `IC_MENU` | `list.bullet.rectangle` |
| FolderLock | `IC_LOCK` | `lock` |
| Bot | `IC_ROBOT` | the OpenAgents mark (custom image) |
| ImagePlus | `IC_IMAGE_SQUARE` | `photo.badge.plus` |
| LogOut | `IC_EXIT_LOGOUT` | `rectangle.portrait.and.arrow.right` |
| Mic | `IC_MIC` | `mic` |
| VenetianMask (blinded) | None | `theatermasks` (web needs a new icon) |
| Github / Twitter | None (brand icons) | None; use `IC_EXTERNAL_LINK` and a text label |
| Tab: chat / profile / settings | `IC_CHATS` / `IC_USERS` / `IC_SETTINGS` | `bubble.left.and.bubble.right` / `person.2` / `gearshape` |

### B5. Gaps and decisions for the owner

Each decision has a recommendation; agents act on it unless the owner says
otherwise.

1. **Which "current app" look wins on native: T3-style or mono ARWES?**
   OpenAgents iOS today uses T3's look: DM Sans, rounded user bubbles
   (radius 20), a glass composer, stock UIKit buttons and fields. The web
   `/chat` uses mono with ARWES frames and Geist. Arcade's design is a frame
   design. Recommendation: keep the native composer and message views as
   they are (shipped and tested), and add the framed-view primitive (B3) for
   buttons, fields, cards, section boxes and other people's messages. That
   brings Arcade's frames over in gray without rebuilding the chat surface.
2. **Non-gray colours in the current native app.** The send disc is blue
   `#346bf1`, links are `#8cb4ff`, and the refresh control uses system green
   and orange. The owner's rule is white and grayscale. Recommendation:
   change the send disc to white with a black arrow, links to `strong` with
   an underline, and the refresh states to white symbols with distinct
   shapes (`checkmark.circle.fill`, `exclamationmark.arrow.triangle.2.circlepath`)
   in one change.
3. **Native has no images, avatars, rows, switches, tabs or sheets.**
   Arcade needs all of them. Recommendation: add projection roles in this
   order: `icon`, `row` (with avatar), `switch`, `image`, `tabs`, `sheet`,
   `menu`, `confirm`. Each maps to a plain UIKit control (B3), and each
   arrives in a TestFlight build, because the projection is native code.
4. **Drawer versus tabs.** Arcade has three tabs plus a 70-px drawer on Home
   that leads to Channels and Channel Manager. Recommendation: drop the
   drawer. Put "Discover channels" and "Manage channels" in the Home
   header's Plus menu, alongside "New chat". Keep three tabs.
5. **The AI persona.** Recommendation: remove the Faerie. The AI row and
   replies use the OpenAgents mark and OpenAgents' own chat (which speaks as
   "we"). The AI chat merges into the account chat that already exists in
   OpenAgents iOS.
6. **Title screen and city art.** Recommendation: no city image. The login
   screen follows the current email-code login, with a "Log in" heading and
   the Nostr key option second. Arcade's "Create Account" / "Login" pair
   becomes one "Log in" flow, because email code creates the account.
7. **Key login copy.** Arcade's "Enter access key" with an nsec field becomes
   a secondary Nostr option under "Log in". Backup's "Private Key (Hex)" /
   "(Nsec)" screen needs a security review in audit 4 before any UI is drawn;
   showing a secret key in a plain field is not a design choice to copy.
8. **Trading UI.** The OfferForm, ListingItem and Listing screens are
   unreachable in Arcade and name fiat currencies and payment apps.
   Recommendation: do not port them. If trading returns, design it on
   Bitcoin and Lightning only.
9. **Reputation suffix and "Rep.: 90%".** The suffix is live but the source
   is Arcade-specific, and "Rep.: 90%" is a hard-coded placeholder.
   Recommendation: drop both until there is a real source.
10. **Haptics.** Arcade has two. Recommendation: use UIKit's defaults
    (context-menu haptic, swipe-complete `UIImpactFeedbackGenerator` light)
    and nothing more.
11. **Copy fixes.** Recommendation: fix the errors listed in A7 while
    porting, use "Log in" everywhere, and move all strings into one place in
    the app's Coax module.

---

## Files read

Arcade (`/Users/christopherdavid/work/arcade`):

- `app.json`; `ios/arcade/Images.xcassets/AppIcon.appiconset/`;
  `ios/arcade/BootSplash.storyboard`
- `app/theme/colors.ts`, `typography.ts`, `spacing.ts`, `timing.ts`,
  `images.ts`, `index.ts`
- `app/components/`: every file listed in A3, including `Frame/*`,
  `ArwesButton/index.tsx`, `TextInput/index.tsx`, `ActivityIndicator/*`,
  `AnimatedDonut/*`, `SwipeableItem/index.tsx`, `BlurredPopup/*`,
  `ReText/*`, `Faerie/*`, `icons/*.svg`
- `app/navigators/AppNavigator.tsx`, `AuthNavigator.tsx`,
  `TabNavigator.tsx`, `TabBar.tsx`, `HudNavigator.tsx`
- `app/screens/`: every screen file listed in A5, plus `AI/*`,
  `ErrorScreen/*`, `SplashScreen/*`
- `app/utils/haptics.ts`, `parser.tsx`, `formatCreatedAt.tsx`,
  `shortenKey.tsx`; `app/hooks/useSendMessage.ts`
- `app/i18n/en.ts`, `ar.ts`, `ko.ts`
- `assets/images/*`, `assets/icons/*`, `assets/fonts/Protomolecule.otf`
- Git log for the TabBar change (`d520b0f`, `756427a`)

OpenAgents:

- `components/spec.md`; `docs/reuse.md`
- `components/skin/mono.coax`, `components/skin/native-chat.coax`
- `components/kit/frames.coax`, `settings.coax`, `listing.coax`,
  `dock.coax`, `controls.coax` (function list), `work.coax` and
  `stage.coax` (function lists)
- `components/core/arwes/frames.coax` (header and function list);
  `components/core/icons.coax` (names)
- `components/layout/native-composer.coax`; `components/fonts/README.md`
- `apps/openagents-ios/README.md`, `home/README.md`, `home/app.coax`
  (builders used)
- `coax/native/README.md`, `coax/native/composer.md`
- `apps/shared/native/NativeControls.swift`, `NativeProgram.swift`,
  `NativeTypes.swift`, `NativeMessage.swift`, `NativeComposer.swift`,
  `NativeTool.swift`
- `apps/openagents-web/chat-web.coax` (header and builders used),
  `nostr-chat.coax` (header and builders used)

## Open questions

1. The reversed author label in `User.tsx` sits at `right: 13`, which may
   overlap the 40-px avatar on own messages. This doc could not confirm it
   without running the app.
2. The exact cyan of the app icon (about `#00C8E0`) is read from pixels; no
   source file names it.
3. Whether `NativeControls`' system colours (`.systemBackground`, `.label`)
   always resolve dark depends on the app's appearance setting, which this
   doc did not check in `project.yml` or `Info.plist`.
4. The Geist font files: `mono.coax` names Geist and Geist Mono, but
   `components/fonts/` holds only DM Sans. Where the web loads Geist from,
   and whether a native copy may ship, is not settled here.
5. Whether the owner wants the framed look on native message bubbles (mono
   `/chat` frames the person's message) or the current rounded T3 bubble.
   B5 decision 1 recommends keeping the current bubble.
6. Arcade's live list rows draw remote avatars from `void.cat` placeholders.
   What OpenAgents iOS shows when a Nostr profile has no picture (initials,
   mark, or blank) is a product choice not made here.
