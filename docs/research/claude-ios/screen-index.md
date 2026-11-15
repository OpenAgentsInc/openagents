# Claude iOS: observed screen index

Observed 2026-11-15 in the iPhone app, version 1.261113.19. This is a
visual reference, not a specification or a claim about every account or app
state. See the [component inventory](component-inventory.md) for a decomposition
of the captured interface into atoms, molecules and organisms. The screenshots are private and remain on the owner's Desktop under
`iPhone Screenshots/Claude Projects/` and `iPhone Screenshots/Claude Code and Chats/`.
The repository does not contain them. Filenames below identify those local
captures; the descriptions omit account details and conversation contents.

The app uses a dark background, light text, rounded cards and pills, orange
accents for active filters and send actions, and blue checks for selected
options. A circular back or close control usually sits at the upper left;
menus often appear as rounded floating panels or bottom sheets. The iPhone
Mirroring frame and cursor are visible in captures, so they are not native
full-resolution screenshots.

## Coverage and limits

The account used for observation had one active example project, no archived
projects, one active Code session, and a long archive. The archive list was
scrolled to its visible end, but most archived sessions were not opened and
individual transcripts were not exhaustively captured. Other signed-in
accounts were not surveyed. Login and onboarding were not inspected because
the app was already signed in. Feature availability, populated lists,
permissions, and error states can differ by account. Several archive-list
captures repeat overlapping rows or the end of the list. Captures show the
state at one moment, not every possible outcome of a control. No account
settings or repositories were intentionally changed, and no messages were
sent. Opening archived sessions cleared their unread indicators.

## Projects

| Capture in Claude Projects | Screen and layout |
| --- | --- |
| `01-projects-list.png` | Projects list: centered title, menu button left, filter right, project rows below, search and a prominent New project button at the bottom. |
| `02-project-filters.png` | A small floating filter menu anchored at the top right lists Yours and Archived; a check marks the current choice. |
| `03-archived-empty.png` | Archived projects empty state: centered icon and message, with New project at the bottom. |
| `04-project-overview.png` | Project detail: back, title, pin and overflow across the top; a description card, file and instruction pills, centered empty session state, and New session at the bottom. |
| `05-project-options.png` | Overflow panel contains Edit details, Archive, and red Delete actions. |
| `06-edit-details.png` | Bottom sheet for project name and description, with close on the left and a check button on the right. |
| `07-project-files.png` | Project Files sheet shows a capacity bar, file tile with a remove control, and a wide Add Files button. |
| `08-add-files-menu.png` | Floating menu presents Take Photo, Choose Photo, Choose File and Add Text. |
| `09-instructions.png` | Instructions sheet has a large multiline field, explanatory text, close and check buttons. Placeholder examples describe tone and expertise. |
| `10-project-new-session.png` | Empty project chat shows the project title in the header and a composer at the bottom. |
| `11-create-project.png` | Create sheet has name and description fields, close and check buttons. |
| `12-project-search.png` | Search field appears along the bottom of the project list with a separate close button. |

## Code and chat

The following filenames are in Claude Code and Chats.

| Capture | Screen and layout |
| --- | --- |
| `01-code-list.png` | Code landing page groups Devices and Sessions; top controls open navigation, scheduled tasks and filters, while search and New session sit at the bottom. |
| `02-code-filter-menu.png` | Status menu lists All, Needs input, Ready for review, Working, Completed and Archived. |
| `03-code-scheduled.png` | Empty scheduled Code tasks page has a back button, filter and New task control. |
| `04-remote-control-setup.png` | Bottom sheet explains the terminal command to connect a device and offers a copy icon. |
| `05-code-new-session.png` | Empty Code composer centers a mascot; environment and repository pills sit above the text area, with add, model, microphone and send controls. |
| `06-environment-picker.png` | Environment sheet lists Auto and Default, with search, help, settings and Create environment controls. |
| `07-environment-settings-top.png` | Default environment editor shows its name, network setting, allowed-domain field and Save changes button. |
| `08-environment-network-menu.png` | Network menu explains None, Limited and Full access. |
| `09-create-environment.png` | New environment form repeats the network and domain fields and offers Create environment and Set up with Claude. |
| `10-repository-menu.png` | Repository pill opens a compact menu for changing repository or branch. |
| `11-repository-picker.png` | Repository sheet separates selected and available repositories, with search and Connect more at the bottom. |
| `12-connect-repositories.png` | Connection introduction explains the GitHub app and offers a large Connect repositories button; installation was not completed. |
| `13-branch-picker.png` | Branch sheet shows the default branch, other branch rows, selection check and search. |
| `14-code-models.png` | Model sheet shows model rows, selected check, Effort and More models. |
| `15-code-effort.png` | Effort sheet lists levels from Low through Ultracode with a check on the current level and an explanatory note. |
| `16-code-more-models.png` | Additional models appear as a vertically stacked list in a sheet. |
| `17-code-context.png` | Add context sheet has photo previews, camera, files, permission and connectors. |
| `18-code-permission-modes.png` | Mode sheet explains Auto, Accept edits and Plan, with a check on the current mode. |
| `19-code-connectors.png` | Connectors sheet contains discovery, an empty state and Browse connectors. |
| `20-code-thread-bottom.png` | Code transcript view has a header with title, context meter and overflow, a scrollable response, PR control and bottom composer. |
| `21-code-thread-menu.png` | Thread menu offers Share, Copy branch, Rename, feedback and Archive. |
| `22-code-usage.png` | Usage sheet displays context and weekly usage bars, percentages and reset information. |
| `23-all-recents.png` | Recents lists conversations with icons and timestamps, plus a search field and New session button. |
| `24-recents-menu.png` | Recents menu offers selection and All, Pinned and Archived filters. |
| `25-archived-chats-top.png` | Archived list shows thread titles, type icons, timestamps and unread dots. |
| `37-chat-new.png` | Standard chat starts with a centered greeting and a bottom composer. Header controls open navigation and a separate chat mode. |
| `38-chat-context.png` | Add to session sheet contains photos, camera, files, project, permission and device rows. |
| `39-chat-project-picker.png` | Project picker has a back control, add button, searchable project list. |
| `40-chat-permission.png` | Standard-chat permission sheet contrasts manual and automatic approval and marks the selected option. |
| `41-chat-devices.png` | Devices sheet lists a desktop and its last-seen status. |
| `42-chat-models.png` | Standard-chat model sheet uses the same stacked choices, selection check and effort controls as Code. |
| `43-archived-disk-space.png`, `45-archived-system76.png`, `46-archived-issue-476.png`, `48-archived-encapsulated.png`, `49-archived-t3code-audit.png`, `50-archived-stopping-early.png` | Examples of archived Code transcripts: title and usage controls above scrollable messages and tool activity, with a fixed archived notice at the bottom. Text, links, code and tool rows vary by conversation. |
| `47-archived-wobbly-blossom.png`, `51-archived-dreamy.png` | Archived transcript views shown without visible message content at capture time; the title and archived notice remain. |
| `44-code-archive-list.png` | Code list with the archived filter selected, displaying session names and device or repository context. |
| `52-archive-list-2.png`, `53-archive-list-3.png`, `archive-list-04.png` through `archive-list-89.png` | Overlapping scroll positions of the full archived list. Each has the same header, thread rows, unread indicators, search and New session button; later captures repeat the list end. They index list positions, not full transcripts. |
| `69-archived-twinkling.png` | Archived transcript showing a user message and a progress label above the fixed archived notice. |

## Account and app settings

| Capture | Screen and layout |
| --- | --- |
| `27-settings-account.png` | Settings starts with account rows and a selected check, then grouped account settings. Account identifiers are visible in the private screenshot. |
| `28-settings-profile.png` | Profile contains photo, name, nickname and instructions fields, a check button and a destructive account action. |
| `29-settings-privacy.png` | Privacy uses explanatory cards and separate toggles for model improvement and voice data. |
| `30-settings-notifications.png` | Notification categories are grouped in cards with an explanation and toggle per row. |
| `31-settings-time-focus.png` | Time and focus provides break-reminder dropdowns and quiet-hour day buttons. |
| `32-settings-usage.png` | Usage shows a progress bar, reset information and controls for optional credits. |
| `33-settings-billing.png` | Billing shows the account plan, Manage subscription and Restore purchases. |
| `34-settings-shared-links.png` | Shared links empty state uses a centered link icon and message. |
| `35-settings-capabilities.png`, `36-settings-capabilities-lower.png` | Capabilities groups switches for artifact and execution features, memory and tool access. The second capture shows lower rows. |
| `54-settings-app-sections.png` | Lower settings list includes Claude Code, connectors, voice, link handling and haptics. |
| `55-settings-claude-code.png` | Code settings groups startup, transcript and code fonts, appearance toggles and environment management. |
| `56-settings-cloud-environments.png` | Cloud environments lists the default environment and a Create environment control. |
| `57-settings-connectors.png` | Connector settings shows discovery and device-specific permissions, followed by an empty connector state and Browse connectors. |
| `58-settings-voice.png` | Voice and dictation combines a horizontal voice chooser, language and speed rows, mode choices and dictation/privacy links. |
| `59-settings-info.png` | Info popover shows app version and links to policies, licenses and support. |

## Other sections

| Capture | Screen and layout |
| --- | --- |
| `60-artifacts-list.png` | Artifacts page has a header, filter, artifact card with thumbnail and sharing status, and bottom search. |
| `61-artifacts-filters.png` | Filter popover combines ownership and sharing options with artifact type choices. |
| `62-artifact-view.png` | Artifact viewer renders the artifact content with back, comments, share and overflow controls. |
| `63-artifact-menu.png` | Overflow menu offers Open in browser and View comments. |
| `64-artifact-comments.png` | Comments sheet has close and add controls and an empty-state message. |
| `65-scheduled-list.png` | Scheduled tasks page shows a centered empty state and a New task button. |
| `66-scheduled-task-draft.png` | New task opens a normal chat with a prefilled, unsent request in the composer. |
| `67-dispatch-setup.png` | Dispatch introduction explains desktop access, with email-link and pairing buttons. |
| `68-dispatch-pairing.png` | Pairing instructions give a numbered desktop setup sequence and a Done button. |
