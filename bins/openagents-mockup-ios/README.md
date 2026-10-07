# OpenAgents Mockup (iOS)

A screens-only copy of the OpenAgents phone app for design work. It draws
every screen, chat card, sheet, and state in
[revision 3 of the wireframe spec](../../docs/product/2026-09-28-app-wireframe.md#revision-3-what-changed)
with fake data, so visuals can be changed quickly without touching the real
app.

Revision 3 puts the whole loop in **Chat with OpenAgents**: the main
menu's one primary is **CHAT WITH OPENAGENTS**, the first run is a guided
chat, and the Gym's old pages are cards inside chat. A tool is tested with a
**test set**, with the tool and without it: "Coder passes 5 of 8 tests
without it, 7 of 8 with it". You **Add to the Gym**, **Check a result** for
another trainer, and earn XP when someone checks yours. Every screen follows
the spec's first rule, **IDIOT PROOF**: one obvious primary action, plain
words only, a "Next:" line, and no dead ends.

- Pure SwiftUI. No Rust, no network, no sign-in, and no code shared with
  the real app in [`bins/openagents-ios`](../openagents-ios/README.md).
  Nothing here can break it, and it builds in seconds.
- Its own app: bundle ID `com.openagents.mockup`, shown on the phone as
  **OpenAgents Mockup**, version 0.1.0. It installs next to the real app.
- Every tap goes somewhere. Work that would be real (a test run, a chat
  reply, another trainer checking your result, Coder on a computer) is
  faked with timers.

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
bins/openagents-mockup-ios/build.sh shots                     # retake every screenshot in verification/
bins/openagents-mockup-ios/build.sh check                     # fail on a banned word in any on-screen string
bins/openagents-mockup-ios/build.sh test                      # tap through FLOW-01, 02, 07, 08 and count the taps
```

`shots` runs `check` first; the list of screenshots is `screenshots.txt`.
`test` runs `UITests/FlowTests.swift`: FLOW-01 must start the first run in
exactly 3 taps (and resume mid-run after a relaunch), FLOW-02 in 2.

`OPENAGENTS_MOCKUP_DEVICE=<simulator UDID>` picks a simulator other than
the booted one.

## Jump to any screen: the Screen index

**Long press the OPENAGENTS logo** on the main menu (on the first-run
screens, long press **STEP 1 OF 3** or **STEP 2 OF 3**). The Screen index
lists, in sections:

- **Flows · start over**: `FLOW-01` … `FLOW-10`, each reset to where the
  flow begins (`FLOW-01` is the very first open).
- **Screens**: `SCR-01` … `SCR-19` (without the retired two), `CIN-01`, and
  `PAT-01`, each with its states.
- **Chat cards**: `CARD-01` … `CARD-07`, each in a chat that asked for it.
- **Sheets**: `SCR-20` Add to the Gym and `SCR-21` Test set.
- **Retired (rev 2)**: `SCR-03` The Gym and `SCR-04` Training, as revision 2
  drew them, so old playtest findings can be compared.

The same IDs work as the `--screen` launch argument, for example
`--screen CARD-04.firstTry` or `--screen SCR-15.firstRun`. The index is a
design tool, not part of the spec.

The first run keeps its furthest step across relaunch (spec `FLOW-01`
rule 1): quit during the cinematic and it reopens at the end card; quit
during the first test and it reopens the first-run chat with the run still
going, or its result.

## Where to change things

| To change | Edit |
| --- | --- |
| Colors, fonts, sizes, spacing, corner radii, shadows, animation | `App/Theme.swift` (every value, in one place) |
| Words, names, numbers, chat answers, the cinematic's subtitles | `App/MockData.swift` |
| Test sets, results (which tests passed each way), the news list, the check waiting, credit rows | `App/MockData.swift` |
| Timings (fake run length, when the simulated check arrives, chat reply speed, cinematic speed) and switches (show "later" rows, the balance pill, Skip on first play, Level up on the first add) | the top of `App/MockData.swift` |
| A shared piece (row button, player card, chip, composer, cards) | `App/Components/` |
| One screen's or sheet's layout | `App/Screens/<spec ID>_<name>.swift` |
| One chat card's layout | `App/Screens/Cards/CARD<nn>_<name>.swift` (the frame they share is `ChatCard.swift`) |
| Placeholder art (Grid, Gym, Coder figure, hero) | `App/Components/Art.swift`, or drop in real art (below) |

Rules of the look (from the spec): black background, white and gray only,
the one primary action per screen is the single white-filled button,
everything else is outlined or gray (in chat, the newest card's button or
offer is the primary); buttons at least 56 pt tall. The spec asks for text
of at least 17 pt; menu row subtitles are 16 pt so "Test a tool, see what's
new, earn XP" fits on one line (see `Theme.Fonts.rowSubtitle`).

Words follow the spec's "Words on screen": tool, test, test set, with and
without the tool, **Try it once**, **Check a result**, **Add to the Gym**.
`build.sh check` greps every on-screen string for the banned words (eval,
suite, case, grader, baseline, benchmark, practice, key, host, …); the two
approved exceptions are listed in `check-words.sh`.

## Your own art

`App/Assets.xcassets` has two empty image slots:

- **HeroImage**: the main menu's hero (Coder looking out over the Grid and
  the Gym). About 3:2 landscape; it's cropped to fill the card.
- **CoderFigure**: Coder on its own, on a transparent background, portrait.
  Used on the menu, Choose your agent, Coder, and the cinematic.

Drag a PNG into the 2x/3x wells in Xcode. When a slot is filled the app
uses it; when it's empty it draws the placeholder in code. The app icon is
`AppIcon.appiconset/AppIcon-1024.png` (1024×1024, no transparency).

Every font is Paper Mono. `project.yml` bundles the four static faces from
`crates/paper-mono/fonts` and registers them under `UIAppFonts`, and
`App/PaperMono.swift` holds `Font.paper`, which `Theme.Fonts` and every
screen use. Paper Mono has no italic, and weights above Bold draw as Bold.

## Screens and spec IDs

One file per spec screen, card, and sheet, named by its ID:

| File | Spec | States in the Screen index |
| --- | --- | --- |
| `SCR01_MainMenu.swift` | `SCR-01` Main menu | normal, loading, checkWaiting, resultChecked, toolAdopted, noRunsLeft, runInProgress, offline, v1Cut |
| `SCR02_ChooseAgent.swift` | `SCR-02` Choose your agent (step 1 of 3) | normal, loading |
| `CIN01_IntroCinematic.swift` | `CIN-01` Intro cinematic | play, and each shot `S01`…`S08` (end card, **LET'S GO**) |
| `SCR05_Result.swift` | `SCR-05` Result (detail) | better, noChange, worse, firstTry, added, addingFailed, confirmed, didntHold |
| `SCR06_LevelUp.swift` | `SCR-06` Level up | withTitle, noTitle |
| `SCR07_Coder.swift` | `SCR-07` Coder (later) | normal, loading, emptyTesting, error |
| `SCR08_AllTools.swift` | `SCR-08` All tools (later) | normal, offline |
| `SCR09_ToolDetail.swift` | `SCR-09` Tool detail (later) | one per tool |
| `SCR10_Rankings.swift` | `SCR-10` Rankings (later) | week, season, empty, offline |
| `SCR11_Profile.swift` | `SCR-11` Profile, with **What you made** (`E10`) | normal, noResults, offline |
| `SCR12_Updates.swift` | `SCR-12` Updates (later) | normal, empty |
| `SCR13_ReportProblem.swift` | `SCR-13` Report a problem (sheet) | form, formFromChat, sent, offlineSaved |
| `SCR14_SetupPrompt.swift` | `SCR-14` Setup prompt (later, sheet) | saveProgress, connectComputer, openWallet, payConfirm |
| `SCR15_NewChat.swift` | `SCR-15` Chat: new chat; `SCR-15.E12` first-run chat | firstRun, firstTime, returning, selectorOpen, computerChosen, computerConnecting, computerOffline, phoneOffline, dailyLimit, fromFirstRun |
| `SCR16_PreviousChats.swift` | `SCR-16` Chat: previous chats | normal, empty, computerOffline |
| `SCR17_Conversation.swift` | `SCR-17` Chat: a conversation (and the first-run chat) | prepared, whoAreYou, streaming, thinking, failed, dispatch, afterRunCoder, noComputer, aboutResult, aboutTool, whatsTheGym, screenChip, testATool, makeATool, howDidMyTestDo, earnXP |
| `SCR18_WrongAnswer.swift` | `SCR-18` Chat: Wrong answer (inline) | idle, confirming, sending, sent |
| `SCR19_CoderOnComputer.swift` | `SCR-19` Chat: Coder on a computer | working, asked, done |
| `SCR20_AddToGym.swift` | `SCR-20` Add to the Gym (sheet) | normal, adding, failed, check, yourTool |
| `SCR21_TestSet.swift` | `SCR-21` Test set (sheet) | readOnly, draft, anotherTrainer |
| `Cards/CARD01_Tool.swift` | `CARD-01` Tool card | ready, notTestedYet, noRunsLeft, offline |
| `Cards/CARD02_Draft.swift` | `CARD-02` Test set draft card | tests, checks, ready |
| `Cards/CARD03_Run.swift` | `CARD-03` Run card (12-second fake run) | running, slow, failed, offline, checking, tryOnce |
| `Cards/CARD04_Result.swift` | `CARD-04` Result card | better, noChange, worse, firstTry, madeBetter, confirmed, didntHold, added |
| `Cards/CARD05_News.swift` | `CARD-05` Gym news card | items, empty |
| `Cards/CARD06_Check.swift` | `CARD-06` Check card | ready, noneWaiting |
| `Cards/CARD07_Credit.swift` | `CARD-07` Credit card | rows, empty |
| `PAT01_States.swift` | `PAT-01` Offline, error, and empty states (a screen, or inside a card) | offline, runFailed, addingFailed, ourError, offlineMenu, offlineFirstRun |
| `Retired/SCR03_Gym.swift`, `Retired/SCR04_Training.swift` | `SCR-03`, `SCR-04`, retired in revision 3 | as revision 2 drew them |
| `ScreenIndex.swift` | (debug) Screen index | |

Comments in each file name the spec's element IDs (`E01`, `E02`, …) next to
the code that draws them. Screens outside the spec that a chip or row can
open (Wallet, Your computers, Identity keys, the Gym in the Verse) show a
labeled blank page.

The flows click through as the spec draws them:

- `FLOW-01`: **CHOOSE CODER** → the cinematic → **LET'S GO** → the first-run
  chat (step 2 of 3) → **START THE TEST** (tap 3) → the run card (step 3 of
  3) → the result card → **ADD TO THE GYM** → `SCR-20` → Level up → the
  main menu.
- `FLOW-02`: the menu → a starter chip → **START THE TEST** (2 taps).
- `FLOW-03` to `FLOW-06`: chips, prepared answers and follow-ups, streamed
  replies, **Run Coder on Studio Mac**, **Wrong answer**, **Share this
  chat**.
- `FLOW-07`: "Help me make a changelog tool" → the question → the answer
  chip → **Looks good** → the draft (`CARD-02`) → **LOOKS GOOD** (the tests)
  → **LOOKS GOOD** (the checks) → **TRY IT ONCE** → **FIRST TRY** → **Make
  test 3 harder** (optional) → **RUN THE FULL TEST SET** → the result →
  `SCR-20`.
- `FLOW-08`: "a check is waiting" → **CHAT WITH OPENAGENTS** → **RUN THE
  CHECK** → **YOU CONFIRMED IT** → `SCR-20`.
- `FLOW-09`: **What's new** → `CARD-05`; each item opens its card.
- `FLOW-10`: after you add a result, another trainer "checks" it
  `MockData.fakeCheckSeconds` later: the XP arrives, the chat shows
  `CARD-07`, and the menu's next line says who confirmed it.

The chat never reads what you type: a typed message gets one generic
streamed reply, and the chips and cards lead to the answers in
`MockData.answers`.

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
