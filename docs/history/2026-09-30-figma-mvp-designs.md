# Figma MVP designs: table of contents

**This document records history.** It indexes the 2024-era OpenAgents chat MVP
design files in the OpenAgents Figma team. Those designs show the agent-store
chat product, with login through X and message limits for free accounts. They
do not describe the current apps. For current direction, see the
[master roadmap](../roadmap.md).

The files are private to the OpenAgents Figma team, so the links below open
only for team members. Each link goes directly to the frame. Node IDs are
included so an agent with the Figma MCP can fetch a screenshot or design
context for any entry.

Surveyed 2026-09-30 through the Figma MCP.

| File | Key | Pages | Status of this index |
| --- | --- | --- | --- |
| [MVP-v1](https://www.figma.com/design/0pu461dcCuhPuG1IDzB6tv/MVP-v1) | `0pu461dcCuhPuG1IDzB6tv` | not read | **Not indexed yet.** The Figma MCP Starter-plan call limit was reached first. |
| [MVP-v2](https://www.figma.com/design/N26ypveihDzZvZEeNehjmK/MVP-v2) | `N26ypveihDzZvZEeNehjmK` | Auth, Interface, Components, Theme | Auth, Interface, and Theme indexed. Components not read because of the call limit. |
| [MVP-v3](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3) | `3Qa2yCpmvRdaYdpGX7nd72` | Auth, Interface, Components, Theme | Fully indexed. |
| [v4](https://www.figma.com/design/Jct4BBCYNyBZ8E92Jry3M2/v4) | `Jct4BBCYNyBZ8E92Jry3M2` | Auth, Interface, Components, Theme | Auth, Interface, and Components are node-for-node identical to MVP-v3. Theme not read because of the call limit. |

All four files use the same page IDs: Auth `4005:7261`, Interface `4005:7258`,
Components `4006:9619`, and Theme `4006:9662`.

## Design summary

- **Look.** Near-black background with a monospace typeface throughout. The
  palette is greys plus four accents: green for success, red for danger,
  amber for Bitcoin, and white for brand. Primary buttons are white with
  black text.
- **Layout.** Desktop frames are 1281×832. A 256px sidebar with Chats and
  Agents tabs sits on the left. The content area has a 64px navbar, a message
  column 768px wide, and the composer at the bottom. Mobile frames are
  393×852, with the sidebar collapsed into the navbar.
- **Flows.** Signed-out users land on "Who would you like to speak with
  today?", which shows the Agent Store (Chat Agents plus LLMs). Joining is a
  "Join OpenAgents / Login with X" modal. The composer has limit states:
  sign-up warning, sign-up required, upgrade warning, upgrade required (an
  Explorer Plan at $10 per month for 100 responses a day), and a wait state
  for upgraded users.
- **Evolution.** MVP-v2 still designed email and password auth: sign up,
  create password, verify email, login, and reset password. It also kept
  ChatGPT reference screenshots next to each screen. MVP-v3 and v4 replace
  all of that with login through X only, and add mobile variants for every
  screen.

## MVP-v3 and v4

Node IDs below are from MVP-v3. The same IDs resolve in v4.

### Interface page: `Chat UI` section (`4063:9613`)

Screens:

| Screen | Desktop | Mobile |
| --- | --- | --- |
| Chat | [`8002:57059`](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=8002-57059) | [`8002:57071`](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=8002-57071) |
| Chat + Modal (Join OpenAgents, Login with X) | [`8003:62954`](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=8003-62954) | [`8003:63561`](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=8003-63561) |
| New Chat (large modal with the Agent Store) | [`8008:28907`](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=8008-28907) | [`8008:28921`](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=8008-28921) |

Component sets on this page:

| Component | Node | Variants |
| --- | --- | --- |
| Composer | [`4063:9969`](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=4063-9969) | Default, Sign up warning, Sign up required, Upgrade warning, Upgrade required, Upgraded wait |
| Navbar | [`4063:9746`](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=4063-9746) | Mode (Chat or OpenAgents) × Logged In × Responsive (Fullscreen or Mobile) |
| Sidebar | [`4067:13113`](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=4067-13113) | Open or Closed × Fullscreen or Mobile |
| _Sidebar Content | [`6067:33139`](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=6067-33139) | Chats or Agents selected × Fullscreen or Mobile |
| Agent Store | [`8007:90076`](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=8007-90076) | Fullscreen, Mobile |

### Auth page

Each flow section contains a desktop row and a mobile row. The `- Proof`
sections repeat the same flows. Each screen there is paired with a `- Proof`
frame that has no children in the metadata, most likely a pasted screenshot
of the built app.

| Section | Flow | Desktop frames | Mobile frames |
| --- | --- | --- | --- |
| [Login](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=8003-85940) (`8003:85940`) | Chat → Login with X → X Auth → Chat | `8003:85942`, `8003:85966`, `8003:85984` (X Auth 📸), `8003:85954` | `8003:85991`, `8003:86015`, `8003:86033` (Twitter Auth), `8003:86003` |
| [Logout](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=8003-86039) (`8003:86039`) | Chat Auth → Account Button Tip → Chat Unauth (home: "Who would you like to speak with today?") | `8003:86041`, `8003:86065`, `8003:86053` | `8003:86083`, `8003:86095`, `8003:86108` |
| [Login - Proof](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=8002-58536) (`8002:58536`) | Login, plus proof screenshots | `8002:58538`, `8002:58562`, `8002:58580`, `8002:58550`; proofs `8005:5185`–`8005:5188` | `8002:58582`, `8002:58606`, `8002:58624`, `8002:58594`; proofs `8005:5180`–`8005:5183` |
| [Logout - Proof](https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=8003-60303) (`8003:60303`) | Logout, plus proof screenshots | `8003:61516`, `8003:61781`, `8008:70997`; proofs `8003:63776`–`8003:63778` | `8003:60349`, `8003:62403`, `8008:71312`; proofs `8003:63779`, `8003:63871`, `8003:63872` |

### Components page

| Section | Component sets (node) |
| --- | --- |
| Buttons (`4008:9775`) | Button `4010:11330` (Primary, Secondary, Ghost, or Danger × Default, Hover, or Disabled × lg or md); List Button `4080:4973`; Button Tip `4067:14612`; Button Selector `6067:32701` |
| Data (`4081:8289`) | Badge `4081:8872` (Primary, Secondary, Handle, or Success × md or sm); X Avatar `8003:63971` |
| Inputs (`4013:7217`) | Text Input `4021:300` (Default, Focus, Focus Filled, Filled, Error Filled); Image Input `6241:7185`; Filepond file upload `6383:9653` |
| Chat (`4065:10933`) | Agent Icon `4043:7714` (User, OpenAI, Anthropic, Gemini, OpenAgents, Mistral, Satoshi, PDF AI, Reader, Brainstorm, Writing, Travel, Art, Icon, each with a disabled state); Chat Message `4065:10936` (User, ChatGPT, Claude, Mistral); Agent Selector `4065:12718`; Agent Tip `6038:19921` |
| Store (`6049:702`) | Agent Button `6049:7803` (Default, Hover); Agent Card `6049:7903` |

### Theme page

| Section | Contents |
| --- | --- |
| Color (`4025:9932`) | Greys (Black, Off Black, Dark Grey, Grey, Light Grey, Off White, White). Accent constants. Role tokens: Layout (Background, Background Secondary), Typography (Heading, Text, Text Secondary), Interactive (Primary, Secondary, Outline, Handle), Special (Brand, Danger, Success, Bitcoin) |
| Typography (`4026:19`) | Heading 1–6; Text xl–xs; Label xl–xs; Brand xl–xs |
| Icons (`4048:8336`) | Logo `4043:7725` (lg, md, or sm × with or without text); Icon `4043:7676` (google, expand-down, share, add, menu, more, edit, trash, clock, link, up, user, logout, open-agents, store, chat, agent, wrench, close, file, x) |

## MVP-v2

### Auth page

MVP-v2 designs email and password auth. Each OpenAgents screen sits next to
the ChatGPT screen it was modelled on (`ChatGPT - …` frames). The table lists
the OpenAgents screens only.

| Section | Screens (node) |
| --- | --- |
| [Auth](https://www.figma.com/design/N26ypveihDzZvZEeNehjmK/MVP-v2?node-id=4068-16810) (`4068:16810`) | **Sign up:** Chat `4068:20153`, Sign up modal `4068:20282`, Create password `4068:20454`, Verify email `4068:20654`, Email verified page `4068:20647`, Chat `4068:20878`. **Login:** Chat `4068:21010`, Login modal `4068:21034`, Chat `4068:21022`. **Reset password:** Chat `4068:24946`, Login `4068:25076`, Reset password modal `4068:25247`, Password reset page `4068:25557`, Password reset success `4068:25565`. **Logout:** Chat `4068:23164`, Chat `4068:23914`, Home `6086:45861` |
| [Auth Update](https://www.figma.com/design/N26ypveihDzZvZEeNehjmK/MVP-v2?node-id=6415-8671) (`6415:8671`) | A revised version of the same flows: Sign up `6415:8725`, Create password `6415:8746`, Verify email `6415:8765`, Email verified page `6415:8789`, Login `6415:8847`, Chat screens `6415:8674`, `6415:8686`, `6415:8701`, `6415:8713`, `6415:8811`, `6415:8835`, Home `6415:8917` |
| [Proofs](https://www.figma.com/design/N26ypveihDzZvZEeNehjmK/MVP-v2?node-id=6400-31440) (`6400:31440`) | The flows again with `Proof …` screenshots of the built app, desktop and mobile: Proof Twitter Auth `6400:31441` and `6400:31587`, Proof Sign Up `6400:31528` and `6400:31586`, Proof Login `6400:31590` and `6400:31591`, Proof X `6400:31643` and `6400:31644`, Proof Home `6400:31547` and `6400:31543`, and a series of Proof Chat frames |

### Interface page

| Section | Contents |
| --- | --- |
| Chat (`4063:9613`) | Component sets only, with no full screens: Composer `4063:9969` (same six modes as v3), Navbar `4063:9746`, Sidebar `4067:13113` (Open or Closed, with no mobile variant yet), _Sidebar Content `6067:33139` |
| Store (`6029:5993`) | Agent Store `6038:20618` (desktop only) |

### Theme page

Same structure as MVP-v3. The icon set does not yet include `x`.

## Gaps

MVP-v1 in full, the MVP-v2 Components page, and the v4 Theme page have not
been read. The Figma MCP Starter-plan call limit was reached. Rerun the
survey once the limit resets and fill in those sections.
