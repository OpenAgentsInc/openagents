# OpenAgents Mockup (iOS)

A screens-only copy of the OpenAgents phone app for design work. It draws
every screen and state in the
[wireframe spec](../../docs/product/2026-09-28-app-wireframe.md) with fake
data, so visuals can be changed quickly without touching the real app.

- Pure SwiftUI. No Rust, no network, no sign-in, and no code shared with
  the real app in [`bins/openagents-ios`](../openagents-ios/README.md).
  Nothing here can break it, and it builds in seconds.
- Its own app: bundle ID `com.openagents.mockup`, shown on the phone as
  **OpenAgents Mockup**, version 0.1.0. It installs next to the real app.
- Every tap goes somewhere. Work that would be real (a training run, a
  chat reply, Coder on a computer) is faked with timers.

## Run it

You need a Mac with Xcode 26 and [XcodeGen](https://github.com/yonaskolb/XcodeGen)
(`brew install xcodegen`).

**In Xcode (best for design work):**

```sh
cd bins/openagents-mockup-ios
xcodegen generate          # makes OpenAgentsMockup.xcodeproj from project.yml
open OpenAgentsMockup.xcodeproj
```

- Pick an iPhone simulator at the top and press Run (⌘R).
- **Previews:** open any file in `App/Screens/` or `App/Components/` and
  show the canvas (⌥⌘↩). Every screen and component has `#Preview`s,
  including its empty, loading, and error states. Edit `Theme.swift` and
  the previews update live.
- Run `xcodegen generate` again after adding or renaming a file.

**From the terminal:**

```sh
bins/openagents-mockup-ios/build.sh sim                       # build, install, launch on the booted simulator
bins/openagents-mockup-ios/build.sh sim --screen SCR-05.worse # open straight on one screen and state
bins/openagents-mockup-ios/build.sh shots                     # retake the screenshots in screenshots/
```

`OPENAGENTS_MOCKUP_DEVICE=<simulator UDID>` picks a simulator other than
the booted one.

## Jump to any screen: the Screen index

**Long press the OPENAGENTS logo** on the main menu (on the very first
screen, long press **STEP 1 OF 3**). The Screen index lists every spec ID
(`SCR-01` … `SCR-19`, `CIN-01`, `PAT-01`) with each of its states; tap one
to open it. **FLOW-01 · start over** resets the demo to the first open.
The same IDs work as the `--screen` launch argument, for example
`--screen SCR-15.firstTime`. The index is a design tool, not part of the
spec.

## Where to change things

| To change | Edit |
| --- | --- |
| Colors, fonts, sizes, spacing, corner radii, shadows, animation | `App/Theme.swift` (every value, in one place) |
| Words, names, numbers, chat answers, the cinematic's subtitles | `App/MockData.swift` |
| Timings (fake training length, chat reply speed, cinematic speed) and switches (show "later" rows, the ₿ pill, Skip on first play) | the top of `App/MockData.swift` |
| A shared piece (row button, player card, chip, composer, cards) | `App/Components/` |
| One screen's layout | `App/Screens/<spec ID>_<name>.swift` |
| Placeholder art (Grid, Gym, Coder figure, hero) | `App/Components/Art.swift`, or drop in real art (below) |

Rules of the look (from the spec): black background, white and gray only,
the one primary action per screen is the single white-filled button,
everything else is outlined or gray; buttons at least 56 pt tall. The spec
asks for text of at least 17 pt; menu row subtitles are 16 pt so "Ask us
anything. No setup needed." fits on one line (see `Theme.Fonts.rowSubtitle`).

## Your own art

`App/Assets.xcassets` has two empty image slots:

- **HeroImage**: the main menu's hero (Coder looking out over the Grid and
  the Gym). About 3:2 landscape; it's cropped to fill the card.
- **CoderFigure**: Coder on its own, on a transparent background, portrait.
  Used on the menu, Choose your agent, Training, Coder, and the cinematic.

Drag a PNG into the 2x/3x wells in Xcode. When a slot is filled the app
uses it; when it's empty it draws the placeholder in code. The app icon is
`AppIcon.appiconset/AppIcon-1024.png` (1024×1024, no transparency).

To use a custom font, add the `.ttf`/`.otf` to `App/`, register it under
`UIAppFonts` in the app's Info.plist settings, and change `Theme.Fonts` to
`Font.custom("Name", size: …)`.

## Screens and spec IDs

One file per spec screen, named by its ID:

| File | Spec | States in the Screen index |
| --- | --- | --- |
| `SCR01_MainMenu.swift` | `SCR-01` Main menu | normal, loading, checkWaiting, noRunsLeft, trainingInProgress, offline, v1Cut |
| `SCR02_ChooseAgent.swift` | `SCR-02` Choose your agent (step 1 of 3) | normal, loading |
| `CIN01_IntroCinematic.swift` | `CIN-01` Intro cinematic | play, and each shot `S01`…`S08` (end card) |
| `SCR03_Gym.swift` | `SCR-03` The Gym (step 2 of 3) | firstRun, returning, checkWaiting, noRunsLeft, loading, offline |
| `SCR04_Training.swift` | `SCR-04` Training (step 3 of 3) | firstRun, running, check, done, slow, failed, offline |
| `SCR05_Result.swift` | `SCR-05` Result | better, noChange, worse, added, addingFailed, checkConfirmed, checkFailed |
| `SCR06_LevelUp.swift` | `SCR-06` Level up | withTitle, noTitle |
| `SCR07_Coder.swift` | `SCR-07` Coder (later) | normal, loading, emptyTesting, error |
| `SCR08_AllTools.swift` | `SCR-08` All tools (later) | normal, offline |
| `SCR09_ToolDetail.swift` | `SCR-09` Tool detail (later) | one per tool |
| `SCR10_Rankings.swift` | `SCR-10` Rankings (later) | week, season, empty, offline |
| `SCR11_Profile.swift` | `SCR-11` Profile | normal, noRuns, offline |
| `SCR12_Updates.swift` | `SCR-12` Updates (later) | normal, empty |
| `SCR13_ReportProblem.swift` | `SCR-13` Report a problem (sheet) | form, formFromChat, sent, offlineSaved |
| `SCR14_SetupPrompt.swift` | `SCR-14` Setup prompt (later, sheet) | saveProgress, connectComputer, openWallet, payConfirm |
| `SCR15_NewChat.swift` | `SCR-15` Chat: new chat | firstTime, returning, selectorOpen, computerChosen, computerConnecting, computerOffline, phoneOffline, dailyLimit, fromFirstRun, fromTraining |
| `SCR16_PreviousChats.swift` | `SCR-16` Chat: previous chats | normal, empty, computerOffline |
| `SCR17_Conversation.swift` | `SCR-17` Chat: a conversation | prepared, whoAreYou, streaming, thinking, failed, dispatch, afterRunCoder, noComputer, aboutResult, aboutTool, goToGym, screenChip |
| `SCR18_WrongAnswer.swift` | `SCR-18` Chat: Wrong answer (inline) | idle, confirming, sending, sent |
| `SCR19_CoderOnComputer.swift` | `SCR-19` Chat: Coder on a computer | working, asked, done |
| `PAT01_States.swift` | `PAT-01` Offline, error, and empty states | offlineGym, trainingFailed, addingFailed, ourError, offlineMenu, offlineFirstRun |
| `ScreenIndex.swift` | (debug) Screen index | |

Comments in each file name the spec's element IDs (`E01`, `E02`, …) next to
the code that draws them. Screens outside the spec that a chip or row can
open (Wallet, Your computers, Identity keys) show a labeled blank page.

The flows click through as the spec draws them: `FLOW-01` (Choose Coder →
cinematic → Gym → Training → Result → Level up → main menu), `FLOW-02` (menu
→ Gym → run), and the chat flows `FLOW-03` to `FLOW-06` (chips, prepared
answers and follow-ups, streamed replies, **Run Coder on Studio Mac**,
**Wrong answer**, **Share this chat**). The chat never reads what you type:
a typed message gets a generic streamed reply, and the suggestion chips
lead to the prepared answers in `MockData.answers`.

## Ship it to TestFlight

The mockup ships as its own TestFlight app, separate from OpenAgents.

```sh
bins/openagents-mockup-ios/build.sh archive   # signed App Store archive
bins/openagents-mockup-ios/build.sh upload    # upload it to App Store Connect
```

`upload` reads `ASC_API_KEY_ID`, `ASC_API_ISSUER_ID`, and
`ASC_API_PRIVATE_KEY_PATH`, or sources the workspace's
`.secrets/appstoreconnect.env` when they're unset. Bump
`CURRENT_PROJECT_VERSION` in `project.yml` (or set
`OPENAGENTS_MOCKUP_BUILD_NUMBER`) for each upload, and `MARKETING_VERSION`
for a new version.

Signing is manual, like the real app: team `HQWSG26L43`, the Apple
Distribution certificate, and the **OpenAgents Mockup App Store**
provisioning profile for `com.openagents.mockup` (both created through the
App Store Connect API on 2026-09-28). The App Store Connect app record
**OpenAgents Mockup** (SKU `openagents-mockup`) must exist before the first
upload; the API can't create one, so it's made once in App Store Connect.
