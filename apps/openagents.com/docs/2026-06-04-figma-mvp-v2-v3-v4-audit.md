# Figma MVP v2, v3, and v4 Audit

Date: 2026-06-04

Sources:

- MVP v2: https://www.figma.com/design/N26ypveihDzZvZEeNehjmK/MVP-v2
- MVP v3: https://www.figma.com/design/3Qa2yCpmvRdaYdpGX7nd72/MVP-v3?node-id=4005-7261
- v4: https://www.figma.com/design/Jct4BBCYNyBZ8E92Jry3M2/v4

This document audits the three supplied Figma files through the Figma MCP
metadata and design-context surfaces. It is intended as a product and
implementation inventory for OpenAgents Autopilot, not as a pixel-export spec. The
Figma MCP generated React/Tailwind reference code for sampled nodes; that code
is treated only as a structural and token reference. OpenAgents product surface remains a Foldkit app
with Tailwind utilities and local UI registry patterns.

## Executive Summary

All three files describe the same OpenAgents chat product family: a dark,
monospace, operational chat shell with a left conversation/agent sidebar,
top navigation, public chat transcript, bottom composer, auth overlays, an
agent store, and a compact reusable component library.

MVP v2 is the earliest and broadest component-system snapshot. It contains the
same top-level page taxonomy as the later files, but its Interface page is
mostly a design-system board rather than a finished screen catalogue. Its Auth
page is large and exploratory: sign up, login, password reset, email
verification, X/Twitter auth proof frames, ChatGPT comparison captures, and
desktop/mobile proof frames all coexist on one broad canvas.

MVP v3 narrows the product story around X login/logout proof flows. The supplied
link targets the Auth page directly. It contains explicit desktop and mobile
sections for login with X, X auth screenshot/proof states, authenticated chat,
logout/account-button tooltip states, and unauthenticated home/agent-store
states. Compared with v2, v3 removes much of the email/password account-system
sprawl from the visible target page and turns the auth work into a focused proof
sequence.

v4 is the most implementation-ready file. It keeps the same Auth, Interface,
Components, and Theme pages, but its Interface page adds assembled full-screen
desktop/mobile compositions: Chat, Chat + Modal, New Chat, Agent Store, and
modal variants. Its Components page expands the asset set with an X avatar and
an X icon. Its Auth page matches the v3 page structure closely, suggesting v4 is
a cleaned-up continuation of v3 rather than a separate product concept.

The biggest implementation signal is that v4 should be treated as the canonical
current design source, v3 as the auth-flow proof board, and MVP v2 as historical
coverage for earlier email/password auth flows and component variants that may
still be useful.

## Shared File Taxonomy

Each file exposes the same top-level Figma pages:

- `4005:7261` - Auth
- `4005:7258` - Interface
- `4006:9619` - Components
- `4006:9662` - Theme

The page IDs are stable across the three files, but the contents are not
identical. This makes cross-file comparison easy: the files are successive
snapshots of one design system, not independent design languages.

## Product Model Across The Files

The product is a ChatGPT-style multi-agent chat interface for OpenAgents.
Common screens and flows include:

- Public chat conversation between `You`, `GPT-4 Turbo`, and `Claude Opus`.
- Agent selector in the navbar, with examples such as `Claude Opus`.
- Login affordance in the navbar, including `Login with` plus X icon in the
  current auth direction.
- Sidebar containing history, chats/agents segmented control, top utility
  buttons, and footer links such as Blog, Changelog, Developer docs, Terms, and
  Privacy.
- Composer placeholder: `Message ChatGPT-4 on OpenAgents...`.
- Composer disclaimer: `Chat agents make mistakes. All chats are public.`
- Agent store/home prompt: `Who would you like to speak with today?`
- Agent store subtitle: `Discover, try, and create AI chat agents... on the
world's openest open AI platform.`
- Premium/pro marker shown as a compact OpenAgents badge on a Claude response.
- Auth overlays that keep the chat shell visible behind a modal blocker.

This is not a marketing-site design. It is a product surface: dense, direct,
dark, and operational.

## Theme Inventory

The Theme page is nearly identical across MVP v2 and v4 and appears stable.
The page defines the visual patterns through Color, Typography, and Icons.
The Figma page text says color is configured by Constants variables and then
aliased into Theme variables; typography is configured through local text
styles.

### Color Roles

The color board has these groups:

- Greys: Black, Off Black, Dark Grey, Grey, Light Grey, Off White, White.
- Layout roles: Background, Background Secondary.
- Typography roles: Heading, Text, Text Secondary.
- Interactive roles: Primary, Secondary, Outline, Handle.
- Special roles: Brand, Danger, Success, Bitcoin.

The v4 sampled design-context code maps the active implementation tokens as CSS
variables:

- `--bg` defaults to black.
- `--bg-secondary` defaults to `#262626`.
- `--outline` defaults to `#525458`.
- `--heading` defaults to white.
- `--primary` defaults to white.
- `--text` defaults to `#d7d8e5`.
- `--text-secondary` defaults to `#8a8c93`.
- `--highlight` defaults to `rgba(255,255,255,0.15)`.

### Typography

The typography board defines:

- Heading 1 through Heading 6.
- Text xl, lg, md, sm, xs.
- Label xl, lg, md, sm, xs.
- Brand xl, lg, md, sm, xs.

The sampled v4 node uses JetBrains Mono throughout:

- Label md: JetBrains Mono Bold, 16px, line height 100%, letter spacing 0.
- Label xs: JetBrains Mono Bold, 12px, line height 100%, letter spacing 0.
- Text md: JetBrains Mono Regular, 16px, line height 100%, letter spacing 0.
- Heading 4: JetBrains Mono Bold, 16px, line height 100%, letter spacing 0.
- Text xs: JetBrains Mono Regular, 12px, line height 100%, letter spacing 0.
- Label lg: JetBrains Mono Bold, 20px, line height 100%, letter spacing 0.
- Text sm: JetBrains Mono Regular, 14px, line height 100%, letter spacing 0.

Implementation note: this lines up with OpenAgents product surface's existing dark mono direction.
Do not translate this into a light, rounded, SaaS-dashboard theme.

### Icon And Logo Set

The icon board includes:

- google
- expand-down
- share
- add
- menu
- more
- edit
- trash
- clock
- link
- up
- user
- logout
- open-agents
- store
- chat
- agent
- wrench
- close
- file
- x in v4

Logo variants include:

- Size lg, md, sm.
- Text true/false variants for each size.

The v4 file adds or preserves the `x` icon for X authentication, which is not
present in the same way in the MVP v2 Theme icon board.

## Component Inventory

The Components page is the main reusable component catalogue. MVP v2 and v4
share most component sets; v4 adds current auth/avatar assets.

### Buttons

Button variants:

- Variants: Primary, Secondary, Danger, Ghost.
- Statuses: Default, Hover, Disabled.
- Sizes: lg and md.

List Button variants:

- Statuses: Default, Hover, Disabled, Heading.
- Sizes: lg and md.

Button Tip:

- Sizes: lg and md.
- Used in auth/logout proof flows as the account or action tooltip.
- Desktop v3/v4 account-button tips are 272px wide by 64px high.
- Mobile tips are 192px wide by 52px high.

Button Selector:

- Selection: First and Second.
- Used as the sidebar Chats/Agents segmented control.
- Current visible states use a dark selected segment inside a `--bg-secondary`
  outer container.

### Data

Badge variants:

- Primary md/sm.
- Secondary md/sm.
- Handle md/sm.
- Success md/sm.

v4 also includes `X Avatar` on the Data board, represented as a 48px symbol.
This corresponds to the X auth direction in v3/v4.

### Inputs

Text Input statuses:

- Default.
- Focus.
- Focus Filled.
- Filled.
- Error Filled.

Image Input statuses:

- Default.
- Filled.

Filepond/File Upload statuses:

- Default.
- Filled.

The error-filled Text Input appears as a 74px-tall control in later auth
compositions, which is visible in v2 Auth and the v4 reset/login flows.

### Chat

Agent Icon variants:

- User, enabled/disabled.
- OpenAI, enabled/disabled.
- Anthropic, enabled/disabled.
- Gemini, enabled/disabled.
- OpenAgents, enabled/disabled.
- Mistral, enabled/disabled.
- Satoshi, enabled/disabled.
- PDF AI, enabled/disabled.
- Reader, enabled/disabled.
- Brainstorm, enabled/disabled.
- Writing, enabled/disabled.
- Travel, enabled/disabled.
- Art, enabled/disabled.
- Generic Icon, enabled/disabled.

Chat Message variants:

- User.
- ChatGPT.
- Claude.
- Mistral.

Agent selector:

- Implemented as a Button instance plus an `Agent Tip` popover component.
- In assembled screens, the navbar selector uses an agent icon, agent name, and
  expand-down icon.

### Store

Agent Button:

- Default.
- Hover.
- Width in the component catalogue is 350px by 96px.

Agent Card:

- 256px by 228px component.

Agent Store:

- MVP v2 Interface has a single 800px by 1036px Agent Store symbol.
- v4 Interface adds explicit responsive variants:
  - Fullscreen: 800px by 1036px.
  - Mobile: 400px by 1744px.

## Interface Page Comparison

### MVP v2 Interface

MVP v2's Interface page is a component-oriented page rather than a screen-ready
page. It contains:

- Chat section with Composer, Navbar, Sidebar, and `_Sidebar Content`.
- Store section with the Agent Store symbol.
- How to Use section with placeholder documentation.

Composer modes:

- Default.
- Sign up warning.
- Sign up required.
- Upgrade warning.
- Upgrade required.
- Upgraded wait.

Navbar modes:

- Chat, logged out, fullscreen.
- Chat, logged out, mobile.
- OpenAgents, logged out, fullscreen.
- OpenAgents, logged out, mobile.
- Chat, logged in, fullscreen.
- Chat, logged in, mobile.
- OpenAgents, logged in, fullscreen.
- OpenAgents, logged in, mobile.

Sidebar modes:

- Open.
- Closed.

Sidebar content selections:

- Chats.
- Agents.

MVP v2 has less explicit responsive treatment for the sidebar than v4. The v2
sidebar component set does not label responsive variants directly in the Sidebar
symbols, while v4 splits them into `Responsive=Fullscreen` and
`Responsive=Mobile`.

### v4 Interface

v4's Interface page is the strongest implementation reference. It contains both
component boards and assembled screens.

Component board content:

- Composer with the same six modes as v2.
- Navbar with logged-in/logged-out and fullscreen/mobile variants.
- Sidebar with Open/Closed and fullscreen/mobile variants.
- `_Sidebar Content` with Chats/Agents and fullscreen/mobile variants.
- Agent Store with fullscreen/mobile variants.

Assembled screen catalogue:

- Chat Desktop: 1281px by 832px.
- Chat Mobile: 393px by 852px.
- Modal Desktop: Chat + Modal, 1281px by 832px.
- Modal Mobile: Chat + Modal, 393px by 852px.
- Big Modal Desktop: New Chat, 1281px by 832px.
- Big Modal Mobile: New Chat, 393px by 852px.

Important screen details:

- Desktop chat uses a 256px sidebar and 1025px content area.
- Desktop navbar height is 64px.
- Desktop composer height is 90px.
- Desktop body area is 678px high.
- Chat messages are centered in a 768px max-width column.
- Mobile chat uses 393px by 852px framing.
- Mobile navbar height is 100px in chat contexts and 52px in some home/unauth
  contexts.
- Mobile sidebar collapses to near-zero width.
- Mobile composer height is 108px.
- Mobile message column width is about 361px with 16px side margins.

The `New Chat` large modal is a key v4 addition. It overlays the Agent Store
inside a modal card:

- Desktop big modal card: 816px by 704px, x=232.5, y=64.
- Mobile big modal card: 361px by 788px, x=16, y=32.
- The Agent Store inside the modal exceeds modal height, implying scrollable
  content is required.

## Auth Page Comparison

### MVP v2 Auth

MVP v2 Auth is the broadest auth exploration board. It includes sign-up,
login, logout, password reset, proof frames, and external ChatGPT comparison
captures.

Main sign-up frames:

- Chat baseline.
- Sign up modal.
- Create password modal.
- Verify email modal.
- Email verified page.
- Proof Sign Up.
- Proof Chat.
- Mobile proof frames.

Sign-up modal:

- Modal over chat shell.
- Modal Card 432px wide.
- Title: `Sign up`.
- Email Text Input.
- Primary Button.
- `or` separator.
- Secondary/social Button.
- Terms copy: `By signing up, you agree to our Terms of Service and Privacy
Policy.`

Create-password modal:

- Title: `Create password`.
- Two Text Input instances.
- Submit Button.

Verify-email modal:

- Title: `Verify your email`.
- Body: `We sent a verification link to satoshi@nakamoto.com.`
- Button.

Email-verified page:

- Standalone page, not chat modal.
- Logo at top and bottom.
- Center text: `Email verified`.
- Body: `Return to your previous session to finish signing up.`

Login frames:

- Chat baseline.
- Login modal.
- Proof Login desktop/mobile.
- X proof frames.

Login modal:

- Title: `Login`.
- Email Text Input.
- Password Text Input.
- Login Button.
- `or` separator.
- Social/X Button.
- Small forgotten/reset action Button.

Password-reset frames:

- Reset password modal that says a password reset link was sent.
- Standalone reset password page with two Text Input instances and Button.
- Password reset success page with `Password reset` and `Return to OpenAgents
to login.`

Logout frames:

- Authenticated chat.
- Account Button Tip.
- Proof Chat.
- Proof Home.
- Home/unauth state with Agent Store.

MVP v2 is useful if OpenAgents product surface needs email/password auth later. It is less useful as
the current implementation source for the X-only direction.

### MVP v3 Auth

The supplied MVP v3 URL targets `Auth` at `4005:7261`. The visible board is much
more focused than v2 and is organized into four large sections:

- Login.
- Login - Proof.
- Logout.
- Logout - Proof.

Each section has desktop and mobile groups.

Login flow:

- Chat baseline.
- Login with X modal.
- X Auth screenshot/proof frame.
- Return-to-chat state.

Login with X modal:

- Desktop frame: 1281px by 832px.
- Mobile frame: 393px by 852px.
- Modal Card desktop: 432px by 260px at x=424.5, y=286.
- Modal Card mobile: 361px by 281px at x=16, y=285.5.
- Title: `Join OpenAgents`.
- Button instance for X login.
- Terms copy: `By logging in with X, you agree to our Terms of Service and
Privacy Policy.`

Logout flow:

- Chat Auth.
- Account Button Tip.
- Chat Unauth.
- Proof counterparts for each.

Chat Unauth/Home:

- Sidebar collapsed to near-zero width.
- Content takes the full frame width.
- Navbar remains visible.
- Body contains a centered Agent Store home layout.
- Desktop Center width is 768px; mobile Center width is about 361px.
- Title: `Who would you like to speak with today?`
- Subtitle: `Discover, try, and create AI chat agents... on the world's
openest open AI platform.`
- Agent Store instance below the title.

v3 is the clearest source for the X auth user journey and for how login/logout
proof frames are arranged. It should be retained as flow evidence even if v4 is
used for implementation.

### v4 Auth

The v4 Auth page has the same high-level taxonomy as v3:

- Login.
- Login - Proof.
- Logout.
- Logout - Proof.

The v4 Auth metadata closely matches v3 for:

- Desktop and mobile Chat baseline frames.
- Login with X modal.
- X Auth proof frames.
- Chat Auth.
- Account Button Tip.
- Chat Unauth/Home.
- Desktop and mobile proof frames.

This strongly suggests v4 carries forward the v3 auth direction. The major
difference from MVP v2 is the de-emphasis of email/password account creation in
the primary visible auth canvas. The major difference from v3 is not in Auth
structure, but in v4's broader Interface and Components pages, which are more
implementation-ready.

## Screen And Layout Inventory

### Desktop Chat Shell

Frame size:

- 1281px by 832px in Figma assembled screens.

Structure:

- Root background uses `--bg` black.
- Sidebar is 256px wide when open.
- Content is 1025px wide when sidebar is open.
- Navbar is 64px tall.
- Body is 678px tall.
- Composer is 90px tall.
- Messages are centered in a 768px wide column.

Navbar:

- Left side holds agent selector.
- Agent selector includes agent icon, label, and expand-down icon.
- Right side holds share/action button and login/account button.
- Login button text in sampled v4 screen: `Login with` plus X icon.

Messages:

- User messages use User agent icon and bold `You` label.
- Assistant messages use brand-specific icons and model names.
- GPT-4 Turbo and Claude Opus appear in sample transcript.
- Claude response includes compact Pro badge with OpenAgents icon.

Composer:

- Text input with placeholder.
- Send/up icon button.
- Disclaimer below input.

### Mobile Chat Shell

Frame size:

- 393px by 852px.

Structure:

- Sidebar collapses to near-zero width.
- Content uses the full mobile width.
- Chat navbar is 100px tall in assembled chat frames.
- Body starts below navbar and is 644px tall in chat frames.
- Composer is 108px tall.
- Message column width is about 361px with 16px margin.

Message positioning:

- Some mobile chat frames position earlier messages with negative y offsets.
  This represents a scrolled conversation state, not content that should render
  above the viewport in production.

### Home / Unauthenticated Agent Store

Desktop:

- Frame 1281px by 832px.
- Sidebar closed.
- Navbar 64px.
- Body 768px or taller depending on source page.
- Center width 768px.
- Title block at y=64.
- Agent Store starts around y=306.

Mobile:

- Frame 393px by 852px.
- Navbar sometimes 52px instead of 100px.
- Body 800px.
- Center width about 361px.
- Title height 372px.
- Agent Store starts around y=484 and extends beyond the viewport.

Implementation note: home/agent-store screens require normal page scrolling or
an internal body scroll container. The mobile Agent Store height is 1744px.

### Auth Modal Pattern

Common structure:

- Modal frame covers the full chat frame.
- Blocker rounded-rectangle covers the full frame.
- Modal Card sits above the blocker.
- Sidebar/content/chat remain present behind the modal structure.

Desktop compact X login modal:

- Card 432px by 260px.
- Centered at x=424.5, y=286 in a 1281px by 832px frame.

Mobile compact X login modal:

- Card 361px by 281px.
- x=16, y=285.5.

Large New Chat modal:

- Desktop card 816px by 704px, containing Agent Store.
- Mobile card 361px by 788px, containing Agent Store.

## Component-To-OpenAgents product surface Mapping

The following mapping is recommended if implementing from these designs in
OpenAgents product surface:

- Figma Button -> local UI registry button primitive, with Tailwind classes for
  variant/status/size.
- Figma List Button -> sidebar history/list row primitive.
- Figma Button Selector -> segmented control for Chats/Agents.
- Figma Button Tip / Agent Tip -> popover/tooltip primitive, but keep content
  explicit in Foldkit model state.
- Figma Badge -> local data-display badge primitive.
- Figma Text Input -> local input primitive with explicit status union.
- Figma Image Input / Filepond -> upload primitive only when the feature exists;
  do not preload upload mechanics into unrelated chat UI.
- Figma Agent Icon -> icon/avatar component keyed by agent brand.
- Figma Chat Message -> message row component with user/agent variant.
- Figma Composer -> bottom composer component with mode union.
- Figma Navbar -> top-bar component with route/auth/agent state.
- Figma Sidebar -> shell sidebar component with responsive closed/open model.
- Figma Agent Store -> store/list surface composed from Agent Button/Card.

Foldkit implementation implications:

- Model auth state as a discriminated union, not booleans:
  `Unauthenticated | AuthenticatingWithX | Authenticated | FailedAuth`.
- Model modal state as a discriminated union:
  `NoModal | LoginWithXModal | NewChatModal | AccountTip`.
- Model composer mode as the Figma union:
  `Default | SignUpWarning | SignUpRequired | UpgradeWarning |
UpgradeRequired | UpgradedWait`.
- Model sidebar selection as `Chats | Agents`.
- Keep selected agent as structured data, not a string-only label.
- Keep mobile/desktop branching in view helpers and CSS breakpoints; do not put
  viewport facts into persistent product state unless needed for behavior.

## Implementation Priorities

1. Use v4 Theme and Components as the current design-system source.
2. Use v4 Interface assembled screens for chat shell, modal, new-chat, mobile,
   and Agent Store layout.
3. Use v3 Auth for flow confirmation around X login/logout proofs.
4. Use MVP v2 Auth only when reconstructing email/password sign-up, login,
   verification, or password-reset flows.
5. Preserve the black/off-black JetBrains Mono system and compact 4px control
   radius.
6. Treat the Figma MCP generated asset URLs as temporary inspection artifacts.
   They expire and should not be embedded in persistent product code or docs.

## Notable Differences By Version

### MVP v2

- Has extensive email/password auth coverage.
- Contains ChatGPT comparison screenshots/proofs.
- Interface page is mainly component specs, not assembled screen catalogue.
- Sidebar responsiveness is less explicit.
- Theme icon board lacks the later explicit X icon seen in v4.
- Components How to Use page includes inspiration references such as ChatGPT
  OpenAI and TX-24 Houston Mono.

### MVP v3

- Focuses Auth around X login.
- Auth is split into Login, Login - Proof, Logout, and Logout - Proof.
- Has explicit desktop/mobile proof frames for X auth and logout/account-button
  tooltip.
- Introduces or emphasizes unauthenticated home/agent-store state after logout.
- More directly relevant than v2 for current auth behavior.

### v4

- Keeps v3 auth structure.
- Expands Interface into assembled responsive screens.
- Adds Big Modal/New Chat compositions.
- Adds Agent Store responsive variants.
- Adds X avatar/icon coverage.
- Best current source for implementation.

## Open Questions

- Whether OpenAgents product surface should implement only X login initially or preserve MVP v2's
  email/password flows for a later auth milestone.
- Whether `ChatGPT-4` in the composer placeholder is intentional brand copy or
  stale placeholder copy that should become agent-specific.
- Whether public-chat disclosure text should remain visible under the composer
  in every composer mode.
- Whether the mobile chat navbar should be 100px in all chat states or collapse
  to 52px for unauthenticated/home states only.
- Whether Agent Store in modal should be internally scrollable or let the page
  body scroll underneath a fixed modal shell.

## Bottom Line

v4 should be treated as the primary build target. It contains the current design
system, responsive component variants, and assembled screen examples needed for
OpenAgents product surface implementation. v3 is the auth proof companion for X login/logout. MVP v2
is a historical/reference board for earlier email/password flows and broad
component coverage.

## OpenAgents product surface Current UI Gap Analysis

This section compares the Figma direction above with the current OpenAgents product surface web UI
implementation in `apps/web`. It is based on the code present in the OpenAgents product surface repo
on 2026-06-04, especially:

- `DESIGN.md`
- `apps/web/src/styles.css`
- `apps/web/src/ui/primitives.ts`
- `apps/web/src/ui/registry.ts`
- `apps/web/src/ui/forms.ts`
- `apps/web/src/ui/data-display.ts`
- `apps/web/src/page/loggedOut/page/home.ts`
- `apps/web/src/page/loggedOut/page/login.ts`
- `apps/web/src/page/loggedOut/page/onboarding.ts`
- `apps/web/src/page/loggedIn/view.ts`
- `apps/web/src/page/loggedIn/page/chat.ts`
- `apps/web/src/page/loggedIn/page/settings.ts`
- `apps/web/src/page/loggedIn/page/billing.ts`
- `apps/web/src/page/loggedIn/page/usage.ts`

### High-Level Fit

OpenAgents product surface already shares the broad design philosophy of the Figma files:

- Dark-only UI.
- Mono-first typography.
- Dense operational surfaces.
- Compact borders, rows, panels, and status strips.
- Avoidance of decorative gradients and card-heavy marketing shells.
- Tailwind utility implementation through a local UI registry.
- A chat/workroom surface as the primary logged-in product area.

However, OpenAgents product surface is not currently implementing the Figma product surface as shown.
The current app is more of an Autopilot workroom/operator console than the
Figma multi-agent public chat and agent-store shell. It has real product
features that the Figma files do not model in detail: team rooms, project rooms,
file attachments, run diagnostics, artifacts, billing, usage telemetry, provider
connections, GitHub login, and settings. Conversely, the Figma files contain a
public multi-agent chat/store experience, X auth, agent cards, and unauthenticated
home states that OpenAgents product surface has either not implemented or has implemented in a
different shape.

The practical conclusion is that OpenAgents product surface should not blindly replace its current
UI with the Figma UI. The better path is to port the Figma visual system,
responsive shell geometry, auth/store concepts, and component taxonomy into the
existing OpenAgents product surface workroom architecture.

### Current OpenAgents product surface UI Foundation

OpenAgents product surface's design contract in `DESIGN.md` is compatible with the Figma direction:

- The repo requires dark-only foundations.
- It prefers pure black, warm off-white text, subtle borders, compact mono
  typography, rails, timelines, composers, docks, and context panes.
- It explicitly rejects light mode, decorative gradients, bokeh, generic
  assistant framing, and card-heavy marketing.
- It prefers Tailwind utilities and the local Foldkit UI registry.

The current CSS foundation in `apps/web/src/styles.css` defines:

- Berkeley Mono as the shipped mono font.
- Commit Mono / IBM Plex Mono as chat mono fallbacks through
  `--font-family-chat-mono`.
- Tailwind theme aliases for background, surface, text, border, and font tokens.
- Dark page root with `html`, `body`, and `#root` locked to full viewport and
  hidden overflow.
- Animation utilities for status morphs, odometer values, text reveal, pane
  open, and progress strips.

Current OpenAgents product surface tokens:

- `--background-base`: `#101010`
- `--background-stronger`: `#151515`
- `--surface-base`: `rgba(255, 255, 255, 0.031)`
- `--surface-base-hover`: `rgba(255, 255, 255, 0.039)`
- `--surface-base-active`: `rgba(255, 255, 255, 0.059)`
- `--text-base`: `rgba(255, 255, 255, 0.618)`
- `--text-weak`: `rgba(255, 255, 255, 0.422)`
- `--text-weaker`: `rgba(255, 255, 255, 0.284)`
- `--text-strong`: `rgba(255, 255, 255, 0.936)`
- `--border-weak-base`: `rgba(255, 255, 255, 0.195)`
- `--border-weaker-base`: `rgba(255, 255, 255, 0.102)`

Figma v4 tokens:

- `--bg`: black.
- `--bg-secondary`: `#262626`.
- `--outline`: `#525458`.
- `--heading`: white.
- `--primary`: white.
- `--text`: `#d7d8e5`.
- `--text-secondary`: `#8a8c93`.
- `--highlight`: `rgba(255,255,255,0.15)`.

Gap:

- OpenAgents product surface's token system is semantically close but not mapped to the Figma token
  names.
- OpenAgents product surface uses warmer `#f1efe8` as its primary text, while Figma uses pure white
  for heading/primary and `#d7d8e5` for body text.
- OpenAgents product surface's global background base is `#101010`, though many surfaces explicitly
  use `#000` or `#010102`. Figma's root shell is more aggressively black.
- OpenAgents product surface currently uses Berkeley/Commit Mono, while Figma sampled nodes use
  JetBrains Mono.

Recommendation:

- Keep OpenAgents product surface's existing design intent, but add a deliberate bridge layer between
  Figma token names and OpenAgents product surface tokens. For example, define local aliases such as
  `--oa-figma-bg`, `--oa-figma-bg-secondary`, `--oa-figma-outline`,
  `--oa-figma-text`, and map them to current or adjusted values.
- Avoid a sudden typography swap unless product direction requires exact Figma
  fidelity. Berkeley/Commit Mono is consistent with OpenAgents product surface's current design
  contract. If exact Figma matching becomes required, introduce JetBrains Mono
  as a controlled font asset rather than ad hoc CSS.

### Current UI Registry Coverage

OpenAgents product surface has a substantial local registry:

- `pageShell`
- `workroomShell`
- `stackedApplicationShell`
- `routeMain`
- `workroomRail`
- `workroomSidebar`
- `workroomMobileSidebar`
- `workroomRouteMain`
- `workroomChatRoute`
- `workroomContent`
- `workroomSplit`
- `workroomTopBar`
- `workroomPanel`
- `panelHeader`
- `keyValueRows`
- `tabBar`
- `codeBlock`
- `workroomTimeline`
- `workroomTimelineMessage`
- `workroomTimelinePart`
- `workroomComposer`
- `workroomFilePanel`
- `workroomMetadataDialog`
- `settingsWorkspacePage`
- `billingCreditsPage`
- `usageTelemetryPage`
- form primitives such as `inputGroup`, `validatedInputGroup`, `textareaGroup`,
  `selectMenu`, and `checkboxList`
- data primitives such as tables, stats, lists, badges, feeds, alerts, and
  empty states

This means OpenAgents product surface does not need a ground-up UI component effort. The missing
work is mostly remapping, renaming, and introducing Figma-specific chat/store
variants where the current registry is optimized for workroom/product
operations.

Figma component coverage status:

- Button: partially covered by `buttonClass`, `compactButton`, `linkButton`,
  `workroomSidebarActionButton`, and action links.
- List Button: partially covered by sidebar nav/session rows.
- Button Tip: partially covered by the account menu and metadata dialog patterns,
  but not represented as the compact Figma tooltip component.
- Button Selector: not directly implemented as the Figma Chats/Agents segmented
  control, though the registry has `tabBar` and sidebar sections.
- Badge: partially covered by badge/data display primitives and status dots.
- Text Input: covered, but with OpenAgents product surface validation/status semantics rather than
  the exact Figma status set.
- Image Input: not implemented as a Figma-aligned component.
- Filepond/File Upload: partially implemented through thread file upload
  actions, but not as a visible Filepond-style component.
- Agent Icon: not implemented as a Figma brand-variant component.
- Chat Message: partially covered by `workroomTimelineMessage`, but with
  different layout behavior.
- Agent Selector: not implemented as the Figma navbar agent selector.
- Agent Tip: not implemented.
- Agent Button: not implemented.
- Agent Card: not implemented.
- Agent Store: not implemented.
- Logo/Icon set: partially covered through text labels, simple symbols, and
  avatar/image handling; not represented as a formal Figma icon component set.

### Logged-Out Surface Gap

Current `apps/web/src/page/loggedOut/page/home.ts` renders a minimal centered
message:

- `Autopilot is a cloud coding agent.`
- `Launches June 4.`

This is much smaller than both the current app capability and the Figma
unauthenticated home/store screen. Figma v3/v4 expect the logged-out public
surface to still feel like the product:

- Full chat shell frame.
- Collapsed sidebar.
- Top navbar.
- Centered agent-store intro.
- Heading: `Who would you like to speak with today?`
- Agent Store cards/buttons below.
- Login with X affordance in the navbar.

Additional drift:

- The existing logged-out scene test expects `Early access is live now by
invitation only.`, while `home.ts` currently renders `Launches June 4.`.
  That is an implementation/test drift independent of the Figma comparison.

Gap:

- Current logged-out home does not expose the Figma Agent Store concept.
- It does not show the chat shell, navbar, sidebar state, or X login affordance.
- It does not act as a product surface; it acts as a temporary launch/maintenance
  page.

Needed to align:

- Replace or route around the temporary home with a Figma-aligned
  unauthenticated shell.
- Add an `AgentStore` registry component using Agent Button/Card primitives.
- Add desktop/mobile home layouts matching v4:
  - Desktop centered 768px column.
  - Mobile centered 361px column.
  - Agent Store content that scrolls past the viewport on mobile.
- Decide whether the home route should remain a maintenance page under a feature
  flag or become the Figma unauthenticated agent-store surface.

### Logged-Out Login/Auth Gap

Current `apps/web/src/page/loggedOut/page/login.ts` is a local Foldkit auth
example:

- Email field.
- Password field.
- Simulated auth command.
- Password must be `password`.
- Copy says: `Local Foldkit auth example. Use any email with password
"password".`
- Back-to-home footer.

This aligns loosely with MVP v2's older email/password auth exploration, but it
does not align with v3/v4's current X-first auth flow.

Figma v3/v4 auth direction:

- Login with X modal over the chat shell.
- Title: `Join OpenAgents`.
- X auth button.
- Terms copy: `By logging in with X, you agree to our Terms of Service and
Privacy Policy.`
- X Auth screenshot/proof frame.
- Account Button Tip for logout/account affordance.

Current OpenAgents product surface auth direction in code:

- Logged-out login is email/password simulation.
- Logged-in session/account uses GitHub-derived session data and account menu.
- Onboarding uses `Log in with GitHub` as the CTA.
- Settings includes provider account connection flows for ChatGPT/Codex account
  connection, not X.

Gap:

- No X login modal.
- No `Join OpenAgents` modal composition.
- No auth modal layered over the chat shell.
- No X icon/avatar component in the local registry.
- No X auth proof/return state.
- Existing login is a prototype/demo auth flow, not product auth.

Needed to align:

- Decide whether production auth should be X, GitHub, WorkOS/OpenAuth, or some
  combination. The Figma current direction says X; the OpenAgents product surface product code says
  GitHub login and provider account connection.
- If X remains desired, add an auth modal state to the logged-out model:
  `NoAuthModal | LoginWithXModal | XAuthInProgress | XAuthFailed`.
- Replace the local email/password demo with a real auth route or a product
  modal.
- Preserve MVP v2 email/password flows only as future account-management
  reference, not the default current login surface.

### Onboarding Surface Gap

Current onboarding is a separate public/marketing-like flow:

- Header with OpenAgents Autopilot and GitHub login.
- Landing headline: `Stop Babysitting Your AI`.
- Funding demo step.
- Coupon code toggle.
- Credit order summary.

This is coherent for an Autopilot early-access onboarding/funding path, but it
does not appear in the Figma MVP v2/v3/v4 files. The Figma files are about chat,
auth, agent store, and the component library; they do not model a funding-first
onboarding flow.

Gap:

- Onboarding is visually related to OpenAgents product surface's dark UI but not represented in the
  Figma system.
- It uses larger hero-scale typography and a two-column funding demo. That is
  outside the compact Figma chat/store language.
- It uses GitHub login, not X login.

Needed to align:

- Either treat onboarding as a separate Autopilot business flow outside the
  Figma MVP chat/store system, or redesign it using the Figma shell.
- If folded into the Figma direction, onboarding should probably become a modal
  or side-panel flow from the Agent Store/home screen rather than a separate
  hero-like page.

### Logged-In Shell Gap

Current logged-in shell:

- Uses `Ui.workroomShell`.
- Desktop grid columns are `280px minmax(0,1fr)`.
- At widths below 1100px, sidebar becomes 232px.
- At widths below 760px, desktop rail disappears and mobile details nav appears.
- Sidebar is an operational workroom sidebar with:
  - Product title.
  - New thread action.
  - Files action.
  - Session sections.
  - Primary nav sections.
  - Footer rows.
  - Account menu.
- Settings surface swaps in a settings-specific sidebar.

Figma shell:

- Desktop frame is 1281px by 832px.
- Open sidebar is 256px.
- Content area is 1025px.
- Navbar is inside the content column.
- Sidebar has top utility buttons, Chats/Agents selector, history, footer links,
  and no separate settings/workroom operational grouping in the visible shell.
- Mobile sidebar collapses to near-zero width rather than becoming a details
  disclosure nav at the top.

Gap:

- OpenAgents product surface sidebar width is 280px, not 256px.
- OpenAgents product surface sidebar is a mature operational navigation and session rail, not the
  Figma Chats/Agents sidebar.
- OpenAgents product surface mobile nav appears as a `<details>` top disclosure, while Figma mobile
  simply removes/collapses the sidebar and uses a mobile navbar.
- OpenAgents product surface settings routes intentionally replace the sidebar taxonomy; Figma has no
  equivalent settings shell.

Needed to align:

- Introduce a Figma-aligned shell variant for public/multi-agent chat surfaces:
  `agentChatShell`, `agentSidebar`, `agentNavbar`, `agentHome`.
- Keep `workroomShell` for Autopilot operator/team/file/billing surfaces unless
  product direction explicitly wants all logged-in surfaces to become the Figma
  chat shell.
- Adjust default open sidebar width to 256px for Figma-aligned routes.
- Implement Chats/Agents segmented control in the sidebar.
- Decide how settings, billing, usage, and files should coexist with the Figma
  shell. They may remain workroom routes rather than forcing them into the Agent
  Store design.

### Chat Timeline Gap

Current chat timeline:

- Uses `workroomTimeline`.
- Conversation is an operational feed.
- Assistant/system messages can render text, tool parts, diffs, files, and run
  artifacts.
- User messages are right-aligned with avatar on the right.
- Assistant messages are constrained with `md:max-w-200` and `2xl:max-w-[1000px]`.
- There is a top progress strip when streaming.
- Empty personal chat says `Start a new Autopilot chat.`
- Team/project rooms show room-specific empty states.

Figma chat timeline:

- Message column is a centered 768px column.
- Every message row has the avatar/icon on the left.
- User message label is `You`, also left-aligned with the rest of the transcript.
- Assistant messages use brand-specific Agent Icons.
- Sample model labels include `GPT-4 Turbo` and `Claude Opus`.
- Claude Pro badge is shown inline near the label.
- No tool/diff/file run-card vocabulary is shown in the basic Figma transcript.

Gap:

- OpenAgents product surface's timeline is functionally richer but visually different.
- User messages are right-aligned in OpenAgents product surface; Figma user messages are left-aligned
  transcript rows.
- OpenAgents product surface avatars are generic/user avatars; Figma uses formal Agent Icon variants.
- OpenAgents product surface assistant/system messages are workroom parts, not simple chat-message
  rows.
- OpenAgents product surface run/tool/diff/file cards have no Figma equivalent.

Needed to align:

- Add a lightweight `agentChatMessage` component for Figma-style public chat
  transcripts.
- Keep `workroomTimelinePart` for Autopilot run/tool/file details; it is needed
  for OpenAgents product surface's actual product.
- Consider a hybrid layout:
  - Public/agent-store chat uses Figma-style rows.
  - Autopilot workroom runs use existing timeline parts.
  - Team rooms use a moderated hybrid with left-aligned messages and run cards.
- Add `AgentIcon` variants for User, OpenAI, Anthropic, OpenAgents, Mistral,
  PDF AI, Reader, Brainstorm, Writing, Travel, Art, and generic Icon.
- Add badge support for model/pro labels inside message headers.

### Composer Gap

Current `workroomComposer`:

- Bottom form with max width 768px.
- Textarea placeholder is `Type your message...` or `Autopilot run is active...`.
- Submit button is text: `Send` or `Running`.
- File upload action can appear as `+`.
- Production mode selector is intentionally commented out.
- No persistent disclaimer under the input.

Figma composer:

- Max width 768px.
- Placeholder: `Message ChatGPT-4 on OpenAgents...`.
- Send control is an icon button with an up/send icon.
- Disclaimer: `Chat agents make mistakes. All chats are public.`
- Modes include Default, Sign up warning, Sign up required, Upgrade warning,
  Upgrade required, and Upgraded wait.
- Required modes can expand the composer to 342-368px height.

Fit:

- The 768px max width is already aligned.
- The basic bottom placement is aligned.
- The visual border/input structure is close enough to port.

Gap:

- No Figma composer mode union.
- No public-chat disclaimer.
- No icon-only send button.
- Placeholder and model naming differ.
- Current composer is Autopilot-run aware, not public-agent-chat aware.
- File upload affordance is minimal and not equivalent to Figma Filepond/Image
  Input components.

Needed to align:

- Add a composer mode union to the relevant model, not a boolean.
- Create a Figma-style composer registry component with:
  - Icon submit button.
  - Disclaimer slot.
  - Warning/required/upgrade states.
  - Disabled/upgraded-wait state.
- Keep current Autopilot composer behavior for run-active states, or map run
  active to `UpgradedWait` only if the product meaning is actually the same.

### Navbar Gap

Current workroom top bar:

- Uses `workroomTopBar` in internal surfaces.
- Logged-in chat route does not have the same agent-selector navbar shown in
  Figma; the workroom sidebar and route chrome carry most navigation.
- Sidebar product title doubles as product navigation.
- Account menu is in the sidebar footer.

Figma navbar:

- Content-column top bar, 64px desktop / 100px mobile in chat contexts.
- Left side has Agent Selector.
- Right side has share/action button and login/account button.
- Logged-in/out and Chat/OpenAgents modes are explicit variants.

Gap:

- No Figma navbar variant set.
- No visible agent selector in chat route.
- Account actions live in the sidebar, not the top right.
- Mobile navbar behavior differs.

Needed to align:

- Add `agentNavbar` with variant fields:
  - `mode: Chat | OpenAgents`
  - `loggedIn: boolean`
  - `responsive: Fullscreen | Mobile`
  - selected agent data
  - login/account action state
- Move or duplicate account affordance into the navbar for Figma-aligned routes.
- Keep sidebar account menu for settings/workroom routes if needed.

### Sidebar Gap

Current sidebar:

- Product label.
- Header actions.
- Session sections generated from model sidebar state.
- Primary nav section.
- Account menu at bottom.
- Footer rows with stats.
- Settings-specific variant.

Figma sidebar:

- Top utility row with menu and new/edit buttons.
- Chats/Agents segmented selector.
- History groups such as Today and Previous 7 Days.
- Rows such as `First date ideas`, `Tim Cook is CEO`, and `Chocolate chip c...`.
- Bottom links: Blog, Changelog, Developer docs, Terms, Privacy.
- Open/closed variants.
- Fullscreen/mobile responsive variants.

Fit:

- OpenAgents product surface already has session sections and row patterns that can represent chat
  history.
- Header actions can represent top utility buttons.
- Footer rows can be replaced with footer links.

Gap:

- No segmented Chats/Agents selector.
- No closed/open sidebar model mapped to Figma's exact variants.
- No mobile near-zero sidebar variant; current mobile uses disclosure nav.
- Bottom account menu and stats differ from Figma footer links.
- Sidebar row typography and spacing are denser than Figma's 16px list rows in
  places.

Needed to align:

- Create an `agentSidebar` variant instead of overloading `workroomSidebar`.
- Give it explicit `mode`, `responsive`, and `selection` fields.
- Reuse existing session sections for `Chats`.
- Add an `Agents` selection showing agent list/store entry points.
- Add footer links matching Figma for public shell.

### Agent Store Gap

Current OpenAgents product surface has no Figma-style Agent Store implementation. The closest
concepts are:

- Team/project room metadata.
- Provider account connections.
- Sidebar session sections.
- Billing/usage/settings pages.

Figma Agent Store is central:

- It appears on unauthenticated home.
- It appears in the New Chat big modal.
- It has fullscreen and mobile variants.
- It composes Agent Button and Agent Card.
- It is the primary way users choose who to speak with.

Gap:

- No agent catalogue data model.
- No Agent Button/Card components.
- No Agent Store page or modal.
- No New Chat modal with Agent Store.
- Current New Thread action starts a thread directly rather than opening the
  Figma big modal.

Needed to align:

- Add an agent catalogue model:
  - id
  - name
  - brand/icon
  - description
  - status/availability
  - premium/pro marker
  - default prompt/thread behavior
- Add `AgentButton`, `AgentCard`, and `AgentStore` registry components.
- Change `ClickedNewChat` behavior for Figma-aligned routes to open a New Chat
  modal instead of immediately creating/resetting the current chat.
- Decide how Autopilot, project agents, ChatGPT/Codex, and provider accounts
  appear inside the store.

### Modal And Overlay Gap

Current OpenAgents product surface modals:

- `workroomMetadataDialog` for run metadata.
- Account menu dropdown in sidebar.
- Some panels/docks but no broad auth/new-chat modal system matching Figma.

Figma modal pattern:

- Full-frame blocker over chat shell.
- Modal Card centered.
- Compact X login modal.
- Large Agent Store/New Chat modal.
- Mobile cards use 16px horizontal margins.

Gap:

- No shared modal state for login/new-chat/account-tip.
- Existing metadata dialog is diagnostics-specific.
- Account Button Tip is not represented as a Figma Button Tip.

Needed to align:

- Add a reusable `modalShell`/`modalCard` registry component with Figma blocker
  and card sizing.
- Add modal state union to the relevant page model.
- Implement `LoginWithXModal`, `NewChatModal`, and `AccountButtonTip` as first
  consumers.
- Keep diagnostics dialog visually distinct or migrate it to the same modal
  primitive with an operator-specific card body.

### Settings, Billing, Usage, Files, And Team Rooms

These surfaces are real OpenAgents product surface product scope but are not deeply represented in
the Figma files.

Current implemented scope:

- Settings workspace with general, connections, organization, members.
- Provider account device-login connection actions.
- Billing credits page with packages, coupon, active runs, recent ledger.
- Usage telemetry page with token totals, leaderboards, recent runs.
- File upload and file detail pages.
- Team and project chat rooms.
- Autopilot run metadata and artifacts.

Figma coverage:

- General component primitives can support these surfaces.
- No explicit settings screen.
- No explicit billing screen.
- No explicit usage/leaderboard screen.
- No explicit file browser.
- No explicit team room/project room.
- No explicit provider account connection screen.

Implication:

- These surfaces should not be judged as "wrong" because Figma does not show
  them. They need a second design pass that extends the v4 theme and component
  system into OpenAgents product surface-specific product workflows.

Needed to align:

- Keep current workroom/operator density.
- Normalize tokens and typography to the Figma/OpenAgents product surface bridge layer.
- Ensure all controls use the Figma component taxonomy where possible:
  Button, List Button, Badge, Text Input, File Upload, Modal, Sidebar, Navbar.
- Avoid adding Agent Store concepts to settings/billing unless there is a clear
  product relationship.

### Implementation Update - v4 Button Extraction

Implemented after this audit:

- Added a separate v4 registry surface in `apps/web/src/ui/v4.ts`.
- Extracted `v4ButtonClass`, `v4Button`, and `v4LinkButton`.
- Covered Figma Button variants: `primary`, `secondary`, `danger`, `ghost`.
- Covered Figma Button sizes: `lg` and `md`.
- Covered Figma Button status naming: `default`, `hover`, `disabled`.
- Reused the v4 button family on the live GitHub auth button rendered by
  `apps/web/src/view.ts`.
- The shared GitHub auth button now appears on both `/` and `/login`.
- The live auth button uses the v4 `secondary` + `md` preset with a larger
  GitHub icon and non-clipping label text.
- The v4 secondary/ghost hover fallback was darkened to
  `rgba(255,255,255,0.08)` after visual review because the original hover gray
  read too light in production.

Important route-ownership note:

- Live `/login` is not rendered by
  `apps/web/src/page/loggedOut/page/login.ts`. It is rendered by
  `apps/web/src/view.ts` through `githubLoginButton`.
- Future route-specific UI work must trace the top-level route/view path before
  editing a same-named page module.

### Implementation Gap Matrix

| Figma concept             | Current OpenAgents product surface status                         | Recommended next step                                      |
| ------------------------- | -------------------------------------------- | ---------------------------------------------------------- |
| Theme color roles         | Semantically close but token names differ    | Add token bridge and migrate hard-coded hexes gradually    |
| JetBrains Mono typography | Uses Berkeley/Commit Mono                    | Decide whether exact font fidelity is required             |
| Button variants           | Implemented as initial v4 registry primitive | Expand usage across remaining action/link buttons          |
| List Button               | Partially covered in sidebar rows            | Extract named primitive                                    |
| Button Tip                | Partial account menu only                    | Add compact tip/popover primitive                          |
| Button Selector           | Not direct                                   | Add segmented selector for Chats/Agents                    |
| Badge                     | Partial                                      | Map Badge variants to current badge/tone system            |
| Text Input states         | Partial                                      | Add exact status union if using Figma forms                |
| Image/File Upload         | Partial/minimal                              | Add Figma-style upload components only for upload surfaces |
| Agent Icon                | Missing                                      | Add brand-variant component                                |
| Chat Message              | Partial but visually different               | Add Figma-style left-aligned message component             |
| Composer                  | Partial                                      | Add Figma composer modes/disclaimer/icon send              |
| Navbar                    | Missing as Figma variant                     | Add agent navbar component                                 |
| Sidebar                   | Partial but workroom-specific                | Add agent sidebar component                                |
| Agent Store               | Missing                                      | Add Agent Store data/model/components                      |
| New Chat modal            | Missing                                      | Add modal state and Agent Store modal                      |
| X login modal             | Missing                                      | Decide auth direction, then implement or discard           |
| Home unauth screen        | Missing                                      | Replace temporary home with Agent Store home if desired    |
| Settings/billing/usage    | Implemented outside Figma                    | Extend theme, do not force exact Figma screen              |

### Recommended Alignment Plan

Phase 1: token and primitive bridge.

- Add Figma/OpenAgents product surface token aliases in `styles.css`.
- Normalize registry hard-coded colors where practical.
- Figma Button primitive started in `apps/web/src/ui/v4.ts`; continue migrating
  high-visibility actions to `v4Button` / `v4LinkButton`.
- Add named primitives for List Button, Badge, Text Input, Button Selector,
  Modal Card, and Agent Icon.
- Keep existing registry APIs stable while adding new Figma-aligned helpers.

Recommended next Figma component extractions, in priority order:

1. `v4TextInput`
   - Reason: Auth, modal, settings, provider-connection, and future Agent Store
     search flows all need a consistent form control.
   - Include status states `default`, `hover`, `active`, `error`, `disabled`,
     and optional leading/trailing icon slots.

2. `v4ModalCard`
   - Reason: The v3/v4 auth and New Chat flows use compact centered modal
     cards. A shared modal primitive prevents every flow from hand-rolling
     border, spacing, and responsive widths.
   - Start with title/body/action/footer slots and desktop/mobile width presets.

3. `v4ButtonSelector`
   - Reason: Figma uses this for Chats/Agents switching. OpenAgents product surface already has
     sidebar route modes that can map to a segmented selector.
   - Include keyboard-accessible tab/segmented-control semantics.

4. `v4ListButton`
   - Reason: Sidebar chat rows, agent rows, file rows, and history rows all need
     one dense selectable row primitive.
   - Include active, hover, disabled, metadata, badge, and optional icon/avatar
     slots.

5. `v4Badge`
   - Reason: Agent status, provider status, plan/tier labels, run status, and
     token usage pages need shared compact labels.
   - Map Figma variants to existing OpenAgents product surface tones instead of creating a second
     unrelated color taxonomy.

6. `v4AgentIcon`
   - Reason: Figma relies on recognizable agent identity marks. This should be
     extracted before Agent Button/Card to avoid one-off icon treatments.
   - Include provider/agent variant, fallback initials, size presets, and status
     dot support.

7. `v4AgentButton`
   - Reason: It is the key reusable row/card seed for Agent Store, New Chat, and
     agent selection.
   - Build from `v4AgentIcon`, `v4Badge`, and `v4ListButton` where possible.

8. `v4Composer`
   - Reason: The chat input is a first-viewport product signal in v4 and should
     not be recreated as a generic textarea.
   - Include mode selector, send action, attachment affordance, disabled state,
     and disclaimer/caption slot.

9. `v4ChatMessage`
   - Reason: Figma chat rows differ from OpenAgents product surface's workroom timeline. A separate
     message primitive lets personal/agent-chat surfaces adopt Figma without
     weakening Autopilot run/tool/diff history.
   - Include author, body, timestamp/status, agent icon, and optional action
     slots.

10. `v4Navbar` and `v4Sidebar`
    - Reason: These should come after the smaller primitives so they compose the
      already-approved controls instead of becoming monolithic layout islands.
    - Keep them shell-level components with slots for account/login, agent
      selector, primary nav, and footer actions.

Phase 2: shell and public home.

- Build `agentChatShell` with 256px desktop sidebar and collapsed mobile sidebar.
- Build `agentNavbar` with agent selector and login/account slot.
- Build `agentSidebar` with Chats/Agents selector and footer links.
- Replace the temporary logged-out home with the Figma unauthenticated Agent
  Store screen, or guard it behind a route/feature flag if launch copy still
  needs to exist.

Phase 3: chat transcript and composer.

- Add Figma-style `agentChatMessage` rows.
- Add `AgentIcon` and model/pro badge support.
- Add Figma composer modes.
- Decide whether personal chat should use the Figma transcript while Autopilot
  run detail remains in workroom timeline cards.

Phase 4: Agent Store and New Chat modal.

- Add a typed agent catalogue.
- Implement Agent Button, Agent Card, Agent Store.
- Change New Thread/New Chat affordance to open the Agent Store modal where the
  Figma flow expects it.
- Use the same Agent Store component for logged-out home and logged-in New Chat.

Phase 5: auth decision and modal.

- Resolve the conflict between Figma X auth and current GitHub/local auth.
- If X auth is product direction, add the X login modal and navbar X login
  button.
- If GitHub remains product direction, adapt the Figma modal pattern but replace
  the X-specific copy/icon with GitHub/OpenAgents account copy. In that case,
  document the deviation from Figma instead of pretending it is implemented.

Phase 6: OpenAgents product surface-only product surfaces.

- Re-skin settings, billing, usage, files, and provider connections through the
  token bridge.
- Keep workroom panels for operational data.
- Add component coverage tests around the new primitives.
- Add scene tests for desktop/mobile home, login modal, new-chat modal, and
  composer modes.

### Highest-Risk Decisions

- Auth provider: Figma says X, current code says GitHub/local simulation plus
  provider account connection. This must be resolved before implementing auth
  screens.
- Product identity: Figma is OpenAgents public multi-agent chat; OpenAgents product surface current
  UI is OpenAgents Autopilot workroom. If both must exist, they need distinct
  route-level shell variants.
- Timeline model: Figma basic chat rows are simpler than OpenAgents product surface's run/tool/diff
  timeline. Replacing the timeline wholesale would remove important Autopilot
  product affordances.
- Mobile navigation: Figma collapses the sidebar; OpenAgents product surface uses a mobile details
  nav. This is a visible behavior difference and should be decided explicitly.
- Agent Store data authority: the UI cannot be meaningfully implemented until
  there is a source of truth for available agents, their status, provider,
  access level, and start-chat behavior.

### Bottom Line For OpenAgents product surface

OpenAgents product surface is directionally compatible with the Figma files but not yet aligned at
the screen/component level. The current repo has a strong dark operational UI
foundation and a rich workroom registry. What it lacks is the Figma v4 public
agent-chat layer: Agent Store, agent navbar, Figma sidebar, X/auth modal, New
Chat modal, brand Agent Icons, left-aligned chat rows, and composer mode states.

The fastest high-quality path is to keep the existing workroom UI for Autopilot
operations and add a Figma-aligned agent-chat shell on top of the current token
and registry system. That preserves OpenAgents product surface's implemented product depth while
bringing the visible chat/home/auth experience in line with v4.
