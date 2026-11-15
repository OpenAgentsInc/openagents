# Claude iOS: observed component inventory

This inventory names the visible building blocks in the captures described by
[the screen index](screen-index.md). Names such as `SettingsPicker` are proposed
labels for discussion, not claims about Claude's source code or component
boundaries. The classification is a design decomposition: an atom is a basic
visual or interactive unit; a molecule combines a few atoms; an organism
assembles a meaningful section or overlay. The same visual pattern can serve
several domains. Screenshots alone cannot establish accessibility semantics,
internal state, animation, exact dimensions or whether similar controls share
an implementation. Only captured states are listed; the coverage limits in the
screen index still apply. `P` references Claude Projects and `C` references
Claude Code and Chats on the owner's Desktop. The images contain private data
and are not included in this repository.

This is a research inventory, not a request to copy an app or introduce
app-private controls. An OpenAgents implementation should first use or extend
[the shared component system](../../../components/spec.md), including its
existing text, edit, Markdown, layout and chat primitives.

## Atoms

| Proposed name | Visible form and variants | Evidence |
| --- | --- | --- |
| `Text` | Primary white, secondary gray, disabled gray, accent blue, destructive red; labels, body, headings, captions and placeholders. The artifact also uses large serif headings. | P04, P06; C20, C55, C62 |
| `Icon` | Line symbols for navigation, files, camera, search, cloud, code, archive, pin, microphone, sharing and more; some use color to convey state. Icons alone do not prove an action. | P04, P08; C01, C18, C57 |
| `BrandMark` | Claude wordmark and orange star or pixel mascot used as identity and empty-state artwork. | C05, C37, C67 |
| `Avatar` | Circular account initials or profile silhouette, including the larger profile-photo placeholder. | C27, C28 |
| `Thumbnail` | Small image preview in a horizontal strip or artifact card; larger embedded images occur in artifact content. | C17, C38, C60, C62 |
| `StatusDot` | Small blue unread dot, distinct from session status icons. | C25, C44 |
| `SelectionCheck` | Blue or white check indicates a selected account, filter, model or option; a check inside a button also represents form confirmation. | P02, P06; C14, C27 |
| `Chevron` | Right-pointing disclosure or down/up picker indicator at the trailing edge of a row. | C07, C27, C54 |
| `Divider` | Thin horizontal separator between rows within cards, menus or transcript sections. | C11, C14, C30 |
| `Badge` | Small text marker such as Recommended, Beta or file type; a short status or context label accompanies other content. | P07; C15, C35 |
| `ProgressBar` | Thin track with colored fill, accompanied by values in its enclosing card. | P07; C22, C32 |
| `Spinner` | Circular activity indicator seen while a view loads or beside work status; its meaning depends on context. | C01, C69 |
| `Slider` | Track with a draggable thumb and small/large text markers for transcript size. | C55 |
| `Switch` | Rounded track and thumb in on, off and visually disabled appearances. A switch is separate from its explanatory row. | C29, C30, C35, C55 |
| `IconButton` | Circular or rounded icon-only button: back, close, menu, filter, search, help, add, pin, overflow, microphone, refresh and info. Fill, outline and active orange vary by context. | P01, P04, P07; C01, C22, C57 |
| `TextButton` | Text or icon-plus-text action, including outlined pills, prominent light filled pills, orange send button, muted disabled action and red destructive action. | P01, P05, P07; C05, C09, C12 |
| `TextField` | Single-line rounded input with placeholder or value; search has a magnifier. | P06, P11, P12; C11 |
| `TextArea` | Rounded multiline input for instructions, description, domains or a message; may show placeholder or a caret. | P06, P09; C07, C37 |
| `Link` | Colored or underlined inline navigation text; external-link icons appear on some menu items. | C20, C29, C59, C62 |
| `InlineCode` | Monospace text in a tinted rounded background within a transcript. | C43, C46 |
| `Bullet` | Bullet marker and spacing for rich-text lists. | C20, C49 |
| `SheetHandle` | Short centered horizontal drag indicator at the top of a bottom sheet. Its presence does not prove a specific gesture was tested. | P07, P09; C06 |
| `Scrim` | Dark translucent layer behind a modal sheet or popover, keeping the underlying screen visible. | P05, P09; C06 |
| `PageIndicator` | Row of dots marking the current item in the voice chooser. | C58 |
| `SystemStatusBar` | Time, connectivity, battery, focus and device cutout at the top; iOS-owned chrome, not an inferred Claude control. | P01; C37 |

## Molecules

| Proposed name | Composition, variants and use | Evidence |
| --- | --- | --- |
| `ScreenHeader` | Centered title with leading navigation and trailing actions; title can have a subtitle or be a rounded session identity pill. | P01, P04; C03, C20 |
| `SheetHeader` | Title centered between close/back and optional confirm, help or add buttons, beneath a handle. | P06, P09; C06, C64 |
| `IconLabel` | Icon plus label, sometimes with secondary text, used in actions and navigation. | P08; C04, C24, C57 |
| `ActionButton` | Label with optional leading icon in filled or outlined pill; includes New session, New project, Add Files, Connect and Done. Width varies from compact to full sheet width. | P01, P07; C12, C68 |
| `SegmentedActionGroup` | Multiple neighboring icon buttons share one rounded container, such as pin/overflow or schedule/filter. | P04; C01, C22 |
| `SearchControl` | Search icon and editable field, optionally with a separate close button; anchored to bottom on several lists. | P12; C13, C23, C60 |
| `FieldWithLabel` | Caption or prompt above a single-line or multiline field, optionally followed by help text. | P06, P11; C07, C28 |
| `SettingsRow` | Leading icon or label, optional description, trailing value and disclosure. Row may be a link, action, or setting; appearance alone does not determine behavior. | C27, C33, C54, C57 |
| `SettingsToggleRow` | Setting title, explanatory copy and trailing switch. Grouped and single-card variants appear. | C29, C30, C35 |
| `SettingsPicker` | Label with current value and disclosure or paired up/down indicator; opens choices in observed network and model examples. | C07, C55, C58 |
| `ChoiceRow` | Primary label, optional secondary explanation or icon, and trailing check for a selected choice. Used for model, effort, permission, environment and branch options. | C06, C13–15, C18, C40 |
| `MenuItem` | Icon and action label in a floating menu; may add a subtitle, check, destructive color or separator. | P05, P08; C02, C21 |
| `AccountRow` | Avatar, account type and secondary identifier or plan, with selection check or overflow. | C27 |
| `ProjectRow` | Project icon, title, truncated description and relative time. | P01, P12 |
| `SessionRow` | Type or status icon, title, secondary context or time, and optional unread dot. Code and Recents use different secondary details. | C01, C23, C25, C44 |
| `RepositoryRow` | Owner and repository name stacked, with a check for selection. | C11 |
| `BranchRow` | Branch name, optional Default caption and selected check. | C13 |
| `DeviceRow` | Device name and last-seen or connection context. | C41 |
| `ArtifactCard` | Thumbnail, title, visibility indicator and edited time arranged as a horizontal card. | C60 |
| `FileTile` | File type badge, filename and separate remove icon in a small rounded tile. | P07 |
| `ContextPill` | Compact rounded control showing a chosen project, environment, repository/branch or model/effort. Its label communicates current context. | P10; C05, C37 |
| `StatusPill` | Compact icon and short status or count; examples include device connection, file count and PR change count. | P04; C20, C44 |
| `InfoCard` | Rounded surface with an icon or heading and supporting text, used for project description or privacy explanation. | P04; C29 |
| `EmptyState` | Centered illustration or icon, short heading and optional explanatory text; often paired with an action elsewhere. | P03, P04; C03, C34, C65 |
| `UsageMeter` | Label, colored bar, current value and reset or capacity caption. | P07; C22, C32 |
| `CommandSnippet` | Monospace command in a compact rounded capsule with a copy action. | C04 |
| `CodeBlock` | Bordered, labeled monospace block with copy and expand controls. | C45 |
| `MessageBubble` | Rounded user-message surface distinct from assistant text; may contain a short command or longer prompt. | C43, C69 |
| `RichTextBlock` | Assistant text assembled from paragraphs, lists, links and inline code; requires shared Markdown and text layout rather than a separate parser. | C20, C46, C49 |
| `MessageActions` | Small copy and playback icons beneath an assistant response. | C20, C43, C45 |
| `TranscriptDateSeparator` | Centered timestamp between decorative horizontal rules. | C43, C69 |
| `ToolActivityRow` | Tool or command summary with icon, truncated text, disclosure and sometimes warning indicator. | C43, C48 |
| `ArchivedNotice` | Fixed dark banner with archive icon and text replacing the active composer in archived sessions. | C43–51, C69 |
| `VoiceChoiceCard` | Large rounded voice tile in a horizontal carousel, with an adjacent partial tile and page dots. | C58 |
| `StepRow` | Number badge and explanatory text in a bordered rounded row, used for pairing instructions. | C68 |
| `PhotoStrip` | Horizontal group of photo thumbnails above attachment actions. | C17, C38 |

## Organisms

| Proposed name | Contents and observed behavior | Evidence |
| --- | --- | --- |
| `AppNavigationDrawer` | Brand, search/gift actions, section links, recent sessions, account control and New session. A partial underlying page remains visible to the right. | Initial capture `claude-04.png`; C23 navigation context |
| `ProjectList` | Header, project rows or empty state, filter, bottom search and create action. | P01–03, P12 |
| `ProjectDetail` | Header with project actions, description, file and instruction controls, session area and New session. | P04–05 |
| `ProjectDetailsForm` | Sheet with name and description fields plus cancel and confirm controls; used for creation and editing. | P06, P11 |
| `ProjectFilesSheet` | Capacity meter, removable file tiles and Add Files action; its add menu is a nested overlay. | P07–08 |
| `ProjectInstructionsSheet` | Multiline instructions editor with contextual help, cancel and confirm. | P09 |
| `CodeSessionList` | Device section and session rows, header navigation and status filter, bottom search and new-session action. | C01–02, C44 |
| `RecentsList` | Conversation rows with type and unread state, a search bar, new-session action and filter/selection menu. Archive is a filtered list, not proof of transcript completeness. | C23–25, C52–53 and archive-list captures |
| `Transcript` | Scrollable rich messages, timestamps, tool activity, inline code and message actions. Active Code adds composer and PR control; archived views use an archived notice. | C20, C43, C45, C48 |
| `ChatComposer` | Multiline message field, attachment action, model/effort pill, microphone and send or voice action. Code adds environment and repository pills; project chat adds project context. | P10; C05, C37–38, C66 |
| `AttachmentSheet` | Photo strip plus camera, file and context-related rows. Code and normal chat expose different additional rows. | C17, C38 |
| `ModelPicker` | Choice rows, selected model, effort submenu and additional-model navigation in a bottom sheet. | C14–16, C42 |
| `PermissionModePicker` | A list of selectable mode rows with descriptions and a check. The available choices differ between Code and normal chat. | C18, C40 |
| `RepositoryPicker` | Selected and available repository groups, search and connection entry point. | C11–12 |
| `BranchPicker` | Scrollable selectable branch rows and bottom search. | C13 |
| `EnvironmentPicker` | Auto and named environments, selected state, search, help, creation and per-environment settings. | C06 |
| `EnvironmentEditor` | Name, network choice, domain input, other configuration below the visible area, and a persistent create or save action. Some lower fields are only partially captured. | C07–09 |
| `RemoteControlSetup` | Modal instructions and copyable command for connecting a device. | C04 |
| `UsagePanel` | Context or account usage meters, reset information and credit controls; the context variant is a sheet. | C22, C32 |
| `SettingsPage` | Account switcher followed by sectioned settings rows, toggles and info popover. | C27, C54, C59 |
| `ProfileEditor` | Avatar, name fields, instruction field, confirmation and destructive account action. | C28 |
| `PrivacySettings` | Explanatory card followed by separate data-use switches. | C29 |
| `NotificationSettings` | Groups of explanatory toggle rows for replies, tasks, Code and product updates. | C30 |
| `TimeFocusSettings` | Break-reminder pickers and quiet-hour weekday controls. | C31 |
| `CapabilitiesSettings` | Grouped feature and memory switches followed by tool access choices. | C35–36 |
| `CodeSettings` | Startup switch, text-size slider, font pickers, appearance switches and cloud-environment link. | C55–56 |
| `ConnectorSettings` | Discovery switch, device permission rows, connector empty state and browsing action. | C19, C57 |
| `VoiceSettings` | Voice carousel, language and speed pickers, mode choices and dictation/privacy links. | C58 |
| `ArtifactLibrary` | Header/filter, search and artifact cards; filter popover groups ownership and type choices. | C60–61 |
| `ArtifactViewer` | Rendered content with navigation, share, comments and overflow; comments appear in a separate sheet. | C62–64 |
| `ScheduledTasksPage` | Empty task state and New task action; the observed action opens an unsent prefilled chat. | C65–66 |
| `DispatchSetup` | Illustration, explanation, email and pairing actions, then a numbered desktop setup screen. | C67–68 |
| `PopoverMenu` | Floating rounded menu anchored near a triggering control; menu items may be checked, grouped or destructive. | P02, P05, P08; C02, C21, C59, C61 |
| `BottomSheet` | Rounded overlay with scrim, handle, header and content; nested sheets support pickers, forms and detail views. The underlying screen is still visible. | P06–09; C06, C14, C38, C64 |
| `FullHeightModal` | Large overlay with header and scrollable or centered content, used where a picker or connection flow needs more space. | C11–12, C27 |

## Observed states and gaps

Selected choices use checks, active filters can use an orange fill, destructive
actions can use red, and disabled controls are muted. Empty project, task,
connector, link and comment states reuse a centered icon and explanatory text;
loading, unread, archived and working are separate visible states. These are
observations, not a complete state machine: focus, keyboard, accessibility,
long-press, error, offline, authentication and most populated variants were
not systematically captured. The archive contains many more transcripts than
were opened, and several captures overlap or repeat list positions. Do not infer
additional components from unseen screens or private conversation content.
