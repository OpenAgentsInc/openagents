import { Button } from "#components/ui/button";
import { Badge } from "#components/ui/badge";
import { Textarea } from "#components/ui/textarea";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandFooter,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandKey,
  CommandList,
  CommandShortcut,
} from "#components/ui/command";
import {
  ComponentValueBinding,
  IntentRef,
  type IconName,
  type IntentError,
  type IntentReporter,
  type JsonPayload,
} from "@effect-native/core";
import { Effect } from "@effect-native/core/effect";
import {
  DesktopApprovalCard,
  DesktopComposerBar,
  DesktopComposerButton,
  DesktopComposerFrame,
  DesktopComposerInput,
  type DesktopApprovalAction,
  type DesktopApprovalDecision,
} from "@openagentsinc/ui/desktop-workbench";
import {
  ArrowDown,
  ArrowUp,
  Bot,
  CheckCircle2,
  Command as CommandIcon,
  Ellipsis,
  File,
  FileDiff,
  Files,
  FolderOpen,
  ImagePlus,
  ListPlus,
  Maximize,
  MessageCircle,
  RefreshCw,
  Send,
  Settings,
  Square,
  SquarePen,
  TerminalSquare,
  X,
  XCircle,
  Zap,
  type LucideIcon,
} from "lucide-react";
import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type DragEvent as ReactDragEvent,
  type ReactElement,
} from "react";

import {
  desktopCommandRegistry,
  formatCommandChord,
  type DesktopCommand,
} from "./command-registry.ts";
import {
  COMPOSER_IMAGE_COUNT_LIMIT,
  canAttachMoreImages,
  composerImageDataUrl,
  formatImageSize,
} from "./composer-images.ts";
import { CODEX_CHIP_REASON_VERIFYING } from "../codex-local-contract.ts";
import { composerActionPresentation } from "../composer-admission.ts";
import { activeFullAutoEnabled, activeFullAutoTurnRunning, capabilityForHarness, formatRelativeTimestamp, questionAnswersReady, type DesktopNoteEntry, type DesktopShellState, type QuestionCardInteraction } from "./shell.ts";
import {
  LexicalComposerEditor,
  type LexicalComposerEditorHandle,
} from "./lexical-composer-editor.tsx";

const composerIconNames = {
  stop: "Stop",
  submit: "ArrowUp",
  attach: "Image",
  remove: "X",
  fullAuto: "Zap",
} as const satisfies Readonly<Record<string, IconName>>;

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(
    report(payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload) as Effect.Effect<void, IntentError>,
  ).catch((error: unknown) => {
    console.error(
      "[openagents-desktop] React composer intent failed",
      name,
      error instanceof Error ? error.message : "unknown intent error",
    );
  });
};

const activeQuestionNote = (state: DesktopShellState): DesktopNoteEntry | null =>
  [...state.notes].reverse().find((note) => note.question?.status === "pending") ?? null;

const questionKind = (
  note: DesktopNoteEntry,
): "provider_question" | "tool_approval" | "plan_review" =>
  note.question?.kind ?? "provider_question";

const commandAvailable = (command: DesktopCommand, state: DesktopShellState): boolean => {
  const note = activeQuestionNote(state);
  const kind = note === null ? null : questionKind(note);
  const capabilities = capabilityForHarness(state);
  if (command.id.startsWith("interaction.question"))
    return kind === "provider_question" && state.questionAnswerHostAvailable && (capabilities?.questions ?? true);
  if (command.id.startsWith("interaction.approval"))
    return kind === "tool_approval" && state.questionAnswerHostAvailable;
  if (command.id.startsWith("interaction.plan"))
    return kind === "plan_review" && state.questionAnswerHostAvailable;
  if (command.id === "chat.send")
    return (
      !state.pending &&
      state.activeThreadId !== null &&
      state.harnessLanes[state.selectedHarness].available
    );
  if (command.id === "chat.stop") return state.pending;
  if (command.id === "chat.steer_current" || command.id === "chat.queue_next")
    return state.pending && state.activeThreadId !== null;
  if (command.id.startsWith("workspace.") && command.id !== "workspace.choose")
    return state.workspaceBrowser.grantRef !== null;
  return true;
};

const commandIcon = (command: DesktopCommand): LucideIcon => {
  if (command.id === "chat.new") return SquarePen;
  if (command.id === "chat.send") return Send;
  if (command.id === "chat.stop") return Square;
  if (command.id === "chat.queue_next") return ListPlus;
  if (command.id === "window.fullscreen_toggle") return Maximize;
  if (command.id === "chat.open") return MessageCircle;
  if (command.id === "workspace.files") return Files;
  if (command.id === "workspace.review") return FileDiff;
  if (command.id === "workspace.choose") return FolderOpen;
  if (command.id === "settings.open") return Settings;
  if (command.id.includes("deny") || command.id.includes("request_changes")) return XCircle;
  if (command.id.includes("approve") || command.id.includes("accept") || command.id.includes("submit")) return CheckCircle2;
  return RefreshCw;
};

const COMMAND_PALETTE_RECENT_SOURCE_LIMIT = 24;

type ComposerDiscoveryItem =
  | Readonly<{ id: string; kind: "command"; label: string; description: string; command: DesktopCommand }>
  | Readonly<{ id: string; kind: "attach-active-file"; label: string; description: string }>
  | Readonly<{ id: string; kind: "path"; label: string; description: string; pathRef: string; pathKind: "file" | "directory" }>
  | Readonly<{ id: string; kind: "skill"; label: string; description: string; invocation: string }>;

/**
 * Bounded composer discovery over already-loaded, typed projections. This is
 * deliberately not a filesystem crawler or keyword router: selecting an item
 * dispatches the same existing intent as the corresponding workbench control.
 */
export const projectComposerDiscoveryItems = (
  state: DesktopShellState,
  query: string,
): ReadonlyArray<ComposerDiscoveryItem> => {
  const normalized = query.trim().toLocaleLowerCase();
  const matches = (value: string): boolean => normalized === "" || value.toLocaleLowerCase().includes(normalized);
  const activeTab = state.workspaceEditor.activePathRef === null
    ? null
    : state.workspaceEditor.tabs.find(tab => tab.pathRef === state.workspaceEditor.activePathRef) ?? null;
  const commands: ReadonlyArray<ComposerDiscoveryItem> = desktopCommandRegistry
    .filter(command => commandAvailable(command, state))
    .filter(command => matches(`${command.label} ${command.id}`))
    .slice(0, 8)
    .map(command => ({
      id: `command:${command.id}`,
      kind: "command" as const,
      label: command.label,
      description: command.id,
      command,
    }));
  const attachActive: ReadonlyArray<ComposerDiscoveryItem> = activeTab?.phase === "ready" && activeTab.document !== null && activeTab.draft.length <= 200_000 && matches(`attach active file ${activeTab.pathRef}`)
    ? [{
        id: `attach:${activeTab.pathRef}`,
        kind: "attach-active-file" as const,
        label: `Attach ${activeTab.pathRef.split("/").at(-1) ?? activeTab.pathRef}`,
        description: `${activeTab.pathRef} · bounded editor context`,
      }]
    : [];
  const seenPaths = new Set<string>();
  const paths: ReadonlyArray<ComposerDiscoveryItem> = state.workspaceBrowser.grantRef === null
    ? []
    : Object.values(state.workspaceBrowser.pages)
        .flatMap(page => page.entries)
        .filter(entry => seenPaths.has(entry.pathRef) ? false : (seenPaths.add(entry.pathRef), true))
        .filter(entry => matches(`${entry.name} ${entry.pathRef}`))
        .slice(0, 12)
        .map(entry => ({
          id: `path:${entry.pathRef}`,
          kind: "path" as const,
          label: entry.name,
          description: entry.kind === "directory" ? `${entry.pathRef} · browse folder` : `${entry.pathRef} · open in editor`,
          pathRef: entry.pathRef,
          pathKind: entry.kind,
        }));
  const skills: ReadonlyArray<ComposerDiscoveryItem> = (capabilityForHarness(state)?.skills ?? false)
    ? state.settings.plugins.plugins
        .filter(plugin => plugin.enabled && plugin.readiness === "ready")
        .flatMap(plugin => plugin.skills.map(skill => ({ plugin, skill })))
        .filter(({ plugin, skill }) => matches(`${plugin.name} ${skill}`))
        .slice(0, 12)
        .map(({ plugin, skill }) => ({
          id: `skill:${plugin.ref}:${skill}`,
          kind: "skill" as const,
          label: skill,
          description: `${plugin.name} · runs on the next turn`,
          invocation: `/skill ${plugin.name}/${skill} `,
        }))
    : [];
  return [...attachActive, ...skills, ...paths, ...commands];
};

export type CommandPaletteSession = Readonly<{
  id: string;
  title: string;
  updatedAt: string;
  intent: "DesktopChatSelected" | "HistoryConversationSelected";
}>;

/**
 * The command palette is a recent-work surface, not a second full-catalog
 * index. Keep its projection bounded even when the loss-accounted history
 * catalog contains years of conversations. Full-catalog lookup remains the
 * explicit session-search path in the rail.
 */
export const projectRecentCommandSessions = (
  state: DesktopShellState,
  query: string,
): ReadonlyArray<CommandPaletteSession> => {
  const normalized = query.trim().toLocaleLowerCase();
  const candidates: ReadonlyArray<CommandPaletteSession> = [
    ...state.threads.slice(0, COMMAND_PALETTE_RECENT_SOURCE_LIMIT).map(thread => ({
      id: thread.id,
      title: thread.title || "Untitled session",
      updatedAt: thread.updatedAt,
      intent: "DesktopChatSelected" as const,
    })),
    ...state.history.catalog.roots.slice(0, COMMAND_PALETTE_RECENT_SOURCE_LIMIT)
      .filter(thread => thread.source === "codex")
      .map(thread => ({
        id: thread.threadRef,
        title: thread.title || "Untitled session",
        updatedAt: thread.updatedAt,
        intent: "HistoryConversationSelected" as const,
      })),
  ];
  const seen = new Set<string>();
  return [...candidates]
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    .filter(session => seen.has(session.id) ? false : (seen.add(session.id), true))
    .filter(session => normalized === "" || session.title.toLocaleLowerCase().includes(normalized))
    .slice(0, 6);
};

const OpenReactCommandPalette = ({
  state,
  report,
}: {
  readonly state: DesktopShellState;
  readonly report: IntentReporter;
}): ReactElement => {
  const [query, setQuery] = useState("");
  const normalized = query.trim().toLocaleLowerCase();
  const commands = useMemo(
    () =>
      desktopCommandRegistry.filter((command) => command.id !== "workspace.review" && commandAvailable(command, state)).filter(
        (command) =>
          normalized === "" ||
          `${command.label} ${command.id}`.toLocaleLowerCase().includes(normalized),
      ),
    [normalized, state],
  );
  const recentSessions = useMemo(
    () => projectRecentCommandSessions(state, normalized),
    [normalized, state.history.catalog.roots, state.threads],
  );
  return (
    <CommandDialog
      open
      onOpenChange={(open) => {
        if (!open) dispatch(report, "DesktopCommandPaletteDismissed");
      }}
      title="Command palette"
      description="Search OpenAgents actions and recent sessions"
    >
      <Command shouldFilter={false} label="Desktop commands">
        <CommandInput
          value={query}
          onValueChange={setQuery}
          placeholder="Search commands and sessions…"
          autoFocus
        />
        <CommandList>
          <CommandEmpty>No matching commands.</CommandEmpty>
          <CommandGroup heading="Actions">
            {commands.map((command) => {
              const Icon = commandIcon(command);
              const chord = formatCommandChord(command.chords, state.host.includes("darwin"));
              return (
                <CommandItem
                  key={command.id}
                  value={command.id}
                  onSelect={() => {
                    dispatch(report, command.intentName, command.payload);
                    dispatch(report, "DesktopCommandPaletteDismissed");
                  }}
                >
                  <Icon aria-hidden="true" />
                  <span>{command.label}</span>
                  {chord === null ? null : <CommandShortcut>{chord}</CommandShortcut>}
                </CommandItem>
              );
            })}
          </CommandGroup>
          {recentSessions.length === 0 ? null : <CommandGroup heading="Recent Sessions">
            {recentSessions.map(session => <CommandItem
              key={`session:${session.id}`}
              value={`session:${session.id}:${session.title}`}
              onSelect={() => {
                dispatch(report, session.intent, session.id);
                dispatch(report, "DesktopCommandPaletteDismissed");
              }}
            >
              <MessageCircle aria-hidden="true" />
              <span className="min-w-0 flex-1 truncate">{session.title}</span>
              <CommandShortcut>{formatRelativeTimestamp(session.updatedAt)}</CommandShortcut>
            </CommandItem>)}
          </CommandGroup>}
        </CommandList>
        <CommandFooter>
          <span className="inline-flex items-center gap-1.5"><CommandKey><ArrowUp /></CommandKey><CommandKey><ArrowDown /></CommandKey>Navigate</span>
          <span className="inline-flex items-center gap-1.5"><CommandKey>Enter</CommandKey>Select</span>
          <span className="inline-flex items-center gap-1.5"><CommandKey>Esc</CommandKey>Close</span>
        </CommandFooter>
        <span className="oa-react-sr-only" role="status" aria-live="polite">
          {commands.length} actions and {recentSessions.length} recent sessions
        </span>
      </Command>
    </CommandDialog>
  );
};

/** Closed palettes do zero command/catalog projection work during startup hydration. */
export const ReactCommandPalette = (props: {
  readonly state: DesktopShellState;
  readonly report: IntentReporter;
}): ReactElement => props.state.commandPaletteOpen
  ? <OpenReactCommandPalette {...props} />
  : <></>;

const ComposerDiscoveryMenu = ({
  state,
  report,
  onClose,
  onRefocus,
}: {
  readonly state: DesktopShellState;
  readonly report: IntentReporter;
  readonly onClose: () => void;
  readonly onRefocus: () => void;
}): ReactElement => {
  const [query, setQuery] = useState("");
  const items = useMemo(() => projectComposerDiscoveryItems(state, query), [query, state]);
  const groups = [
    { kind: "attach-active-file", label: "Current context" },
    { kind: "skill", label: "Skills" },
    { kind: "path", label: "Files and folders" },
    { kind: "command", label: "Commands" },
  ] as const;
  const select = (item: ComposerDiscoveryItem): void => {
    if (item.kind === "command") dispatch(report, item.command.intentName, item.command.payload);
    if (item.kind === "attach-active-file") dispatch(report, "DesktopEditorFileAttached");
    if (item.kind === "path") dispatch(report, "WorkspaceBrowserEntrySelected", item.pathRef);
    if (item.kind === "skill") {
      const current = state.input.startsWith("/skill ") ? state.input.replace(/^\/skill\s+\S+\s*/u, "") : state.input;
      dispatch(report, "DesktopInputChanged", `${item.invocation}${current}`);
    }
    onClose();
    queueMicrotask(onRefocus);
  };
  return (
    <div className="oa-react-composer-discovery" role="dialog" aria-label="Composer commands and context"
      onKeyDown={(event) => {
        if (event.key !== "Escape") return;
        event.preventDefault();
        onClose();
        queueMicrotask(onRefocus);
      }}>
      <Command shouldFilter={false} label="Composer commands and context">
        <CommandInput
          autoFocus
          value={query}
          onValueChange={setQuery}
          placeholder="Search commands, files, folders, and skills…"
        />
        <CommandList>
          <CommandEmpty>
            {state.workspaceBrowser.grantRef === null
              ? "No match. Choose a workspace to discover files and folders."
              : "No matching command or context."}
          </CommandEmpty>
          {groups.map(group => {
            const grouped = items.filter(item => item.kind === group.kind);
            if (grouped.length === 0) return null;
            return <CommandGroup key={group.kind} heading={group.label}>
              {grouped.map(item => {
                const Icon = item.kind === "skill" ? Bot : item.kind === "path"
                  ? item.pathKind === "directory" ? FolderOpen : File
                  : item.kind === "attach-active-file" ? File : commandIcon(item.command);
                return <CommandItem key={item.id} value={item.id} onSelect={() => select(item)}>
                  <Icon aria-hidden="true" />
                  <span className="oa-react-composer-discovery-label">{item.label}</span>
                  <span className="oa-react-composer-discovery-description">{item.description}</span>
                </CommandItem>;
              })}
            </CommandGroup>;
          })}
        </CommandList>
        <CommandFooter>
          <span>Loaded workspace context only</span>
          <button type="button" onClick={onClose}>Close</button>
        </CommandFooter>
      </Command>
    </div>
  );
};

export const ReactComposer = ({
  state,
  report,
}: {
  readonly state: DesktopShellState;
  readonly report: IntentReporter;
}): ReactElement => {
  const editorRef = useRef<LexicalComposerEditorHandle>(null);
  const [dragActive, setDragActive] = useState(false);
  const [discoveryOpen, setDiscoveryOpen] = useState(false);
  const [secondaryControlsOpen, setSecondaryControlsOpen] = useState(false);
  const lastSubmitRef = useRef<Readonly<{ value: string; at: number }> | null>(null);
  const sessionKey = state.activeThreadId ?? state.history.page?.selectedThreadRef ?? "new";
  const lane = state.harnessLanes[state.selectedHarness];
  const capabilities = capabilityForHarness(state);
  const capabilityAdmitted = capabilities === null || capabilities.admission === "admitted";
  const selectedModel = state.selectedHarness === "codex" ? state.codexModel : state.claudeModel;
  const visibleModels = capabilities?.models ?? [selectedModel];
  const selectedModelIndex = Math.max(0, visibleModels.indexOf(selectedModel));
  const nextModel = visibleModels[(selectedModelIndex + 1) % visibleModels.length] ?? selectedModel;
  const otherHarness = state.selectedHarness === "codex" ? "fable" : "codex";
  const otherCapabilities = capabilityForHarness(state, otherHarness);
  const canSwitchHarness = !state.pending && !activeFullAutoTurnRunning(state) && state.harnessLanes[otherHarness].available &&
    (otherCapabilities === null || otherCapabilities.admission === "admitted");
  // FA-H4 (#8877): main reports a BACKGROUND Full Auto turn running on the
  // active thread (renderer non-pending — no live events reach it). Renders
  // the running badge and the Stop control; the send handler fences manual
  // submits while this holds.
  const fullAutoRunning = activeFullAutoTurnRunning(state);
  const activeSubmitMode = fullAutoRunning ? "queue" : state.pendingSubmitMode;
  const pendingAction = composerActionPresentation(state.composerAdmission, activeSubmitMode);
  const alternatePendingMode = state.pendingSubmitMode === "steer" ? "queue" : "steer";
  const alternatePendingAction = composerActionPresentation(state.composerAdmission, alternatePendingMode);
  const alternatePendingModeSupported = alternatePendingMode === "steer"
    ? capabilities?.steerTurn ?? true
    : capabilities?.queueFollowup ?? true;
  const canTogglePendingMode = alternatePendingModeSupported && alternatePendingAction.enabled;
  const hasText = state.input.trim() !== "";
  const hasBoundedContext = state.composerImages.length > 0 || state.composerReviewContext !== null || state.composerFileContext !== null || state.composerTerminalContext !== null;
  const canSubmit = state.pending || fullAutoRunning
    ? state.activeThreadId !== null && hasText && pendingAction.enabled
    : lane.available && capabilityAdmitted && (hasText || hasBoundedContext);
  const atImageLimit = !canAttachMoreImages(state.composerImages);
  const imageSupported = capabilities?.images ?? true;
  const attachmentDisabled = state.pending || fullAutoRunning || atImageLimit || !imageSupported;
  const attachmentLabel = state.pending || fullAutoRunning
    ? "Attach images after the current turn finishes"
    : !imageSupported
      ? `${capabilities?.displayName ?? "This lane"} does not support image attachments`
    : atImageLimit
      ? `Image limit reached (${COMPOSER_IMAGE_COUNT_LIMIT} max)`
      : "Attach images";
  useEffect(() => {
    if (attachmentDisabled) setDragActive(false);
  }, [attachmentDisabled]);
  useEffect(() => {
    setDiscoveryOpen(false);
    setSecondaryControlsOpen(false);
  }, [sessionKey]);
  const submitIntentFor = (pendingMode: "steer" | "queue") => state.pending || fullAutoRunning
    ? pendingMode === "steer"
      ? "DesktopSteerCurrentRequested"
      : "DesktopQueueNextRequested"
    : "DesktopNoteSubmitted";
  const submitLabel = state.pending || fullAutoRunning
    ? activeSubmitMode === "steer"
      ? "Steer"
      : "Queue"
    : "Send";
  const submit = (editorValue = state.input, pendingMode = activeSubmitMode): void => {
    const nextHasText = editorValue.trim() !== "";
    const submitIntent = submitIntentFor(pendingMode);
    const submissionKey = nextHasText
      ? editorValue
      : `context:${state.composerImages.length}:${state.composerReviewContext?.path ?? ""}:${state.composerFileContext?.path ?? ""}:${state.composerTerminalContext?.sessionRef ?? ""}`;
    const now = Date.now();
    if (lastSubmitRef.current?.value === submissionKey && now - lastSubmitRef.current.at < 350)
      return;
    lastSubmitRef.current = { value: submissionKey, at: now };
    // Image-only turns use the intent's explicit null branch so the Effect
    // handler falls back to its attachment-bearing state. An empty component
    // value is not a meaningful message payload. The Effect handler remains
    // the authoritative admission check for text, attachments, lane state,
    // and pending-turn behavior; the button's disabled state is presentation.
    dispatch(report, submitIntent, nextHasText ? editorValue : null);
  };
  useLayoutEffect(() => {
    // A session transition is an explicit keyboard-flow reset. Focus may
    // still be owned by the New session button when this commit lands, so the
    // guarded "unowned focus" rule used for background hydration is wrong
    // here: the composer must synchronously take focus for immediate typing.
    editorRef.current?.focusAtEnd();
  }, [sessionKey]);
  const showDragTarget = (event: ReactDragEvent<HTMLElement>): void => {
    if (!attachmentDisabled && [...event.dataTransfer.types].includes("Files")) setDragActive(true);
  };
  const hideDragTarget = (event: ReactDragEvent<HTMLElement>): void => {
    if (event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)) return;
    setDragActive(false);
  };
  return (
    <DesktopComposerFrame
      data-drag-active={dragActive ? "true" : "false"}
      aria-label="Message composer"
      onSubmit={(event) => {
        event.preventDefault();
        submit(editorRef.current?.readValue() ?? state.input);
      }}
      onDragEnter={showDragTarget}
      onDragOver={showDragTarget}
      onDragLeave={hideDragTarget}
      onDrop={() => setDragActive(false)}
    >
      {state.composerQueue.length === 0 ? null : (
        <section className="oa-react-composer-queue-panel" aria-labelledby="composer-queue-heading">
          <header className="oa-react-composer-queue-header">
            <ListPlus aria-hidden="true" />
            <span id="composer-queue-heading">Queued messages</span>
            <Badge variant="outline">{state.composerQueue.length}</Badge>
          </header>
          <ol className="oa-react-composer-queue">
            {state.composerQueue.map(entry => {
              const editable = entry.status === "queued";
              const status = entry.status === "queued" ? "pending" : entry.status === "promoting" ? "dispatching" : entry.status === "promoted" ? "settled" : entry.status;
              return (
                <li key={entry.queueRef} data-queue-status={status}>
                  <span className="oa-react-composer-queue-order">{entry.position > 0 ? `#${entry.position}` : "—"}</span>
                  <span className="oa-react-composer-queue-message" title={entry.message}>{entry.message}</span>
                  <Badge variant="outline">{status}</Badge>
                  <Button type="button" variant="ghost" size="sm" disabled={!editable}
                    title={editable ? "Edit queued turn" : "This turn is already dispatching"}
                    onClick={() => dispatch(report, "DesktopQueuedIntentEditRequested", entry.queueRef)}>Edit</Button>
                  <Button type="button" variant="ghost" size="sm" disabled={!editable}
                    title={editable ? "Remove queued turn" : "This turn is already dispatching"}
                    onClick={() => dispatch(report, "DesktopQueuedIntentCancelRequested", entry.queueRef)}>Remove</Button>
                </li>
              );
            })}
          </ol>
        </section>
      )}
      {state.composerReviewContext === null && state.composerFileContext === null && state.composerTerminalContext === null ? null : (
        <div className="oa-react-composer-contexts" role="list" aria-label="Attached message context">
          {state.composerReviewContext === null ? null : (
            <div className="oa-react-composer-context" role="listitem" data-context-kind="review">
              <FileDiff aria-hidden="true" />
              <span>
                <strong>{state.composerReviewContext.path}</strong>
                <small>{state.composerReviewContext.source} · {state.composerReviewContext.hunkCount} {state.composerReviewContext.hunkCount === 1 ? "hunk" : "hunks"}</small>
              </span>
              <Button type="button" variant="ghost" size="icon-sm"
                aria-label={`Remove review context for ${state.composerReviewContext.path}`}
                onClick={() => dispatch(report, "DesktopReviewContextRemoved")}>
                <X aria-hidden="true" />
              </Button>
            </div>
          )}
          {state.composerFileContext === null ? null : (
            <div className="oa-react-composer-context" role="listitem" data-context-kind="file">
              <File aria-hidden="true" />
              <span>
                <strong>@{state.composerFileContext.path}</strong>
                <small>{state.composerFileContext.languageMode} · {state.composerFileContext.dirty ? "unsaved draft" : "workspace revision"}</small>
              </span>
              <Button type="button" variant="ghost" size="icon-sm"
                aria-label={`Remove mentioned file ${state.composerFileContext.path}`}
                onClick={() => dispatch(report, "DesktopFileContextRemoved")}>
                <X aria-hidden="true" />
              </Button>
            </div>
          )}
          {state.composerTerminalContext === null ? null : (
            <div className="oa-react-composer-context" role="listitem" data-context-kind="terminal">
              <TerminalSquare aria-hidden="true" />
              <span>
                <strong>{state.composerTerminalContext.shellLabel}</strong>
                <small>{state.composerTerminalContext.cwdLabel} · {state.composerTerminalContext.status}</small>
              </span>
              <Button type="button" variant="ghost" size="icon-sm"
                aria-label={`Remove terminal output for ${state.composerTerminalContext.shellLabel}`}
                onClick={() => dispatch(report, "DesktopTerminalContextRemoved")}>
                <X aria-hidden="true" />
              </Button>
            </div>
          )}
        </div>
      )}
      {state.composerImages.length === 0 ? null : (
        <div className="oa-react-composer-images" role="list" aria-label="Attached images">
          {state.composerImages.map((attachment) => (
            <figure className="oa-react-composer-image" role="listitem" key={attachment.id}>
              <img data-en-key={`composer-image-preview-${attachment.id}`} src={composerImageDataUrl(attachment)} alt="" />
              <figcaption>
                <span title={attachment.name}>{attachment.name}</span>
                <small>{formatImageSize(attachment.sizeBytes)}</small>
              </figcaption>
              <Button
                className="oa-react-composer-image-remove"
                type="button"
                variant="secondary"
                size="icon-sm"
                onClick={() => dispatch(report, "DesktopComposerImageRemoved", attachment.id)}
                aria-label={`Remove ${attachment.name}`}
                title={`Remove ${attachment.name}`}
              >
                <X data-icon-name={composerIconNames.remove} aria-hidden="true" />
              </Button>
            </figure>
          ))}
        </div>
      )}
      {state.composerImageNotice === null ? null : (
        <p className="oa-react-composer-image-notice" role="alert" aria-live="polite">
          {state.composerImageNotice}
        </p>
      )}
      {dragActive ? <span className="oa-react-composer-drop-target" role="status">Drop images to attach</span> : null}
      {discoveryOpen ? <ComposerDiscoveryMenu
        state={state}
        report={report}
        onClose={() => setDiscoveryOpen(false)}
        onRefocus={() => editorRef.current?.focusAtEnd()}
      /> : null}
      <DesktopComposerInput>
        <LexicalComposerEditor
          editorRef={editorRef}
          value={state.input}
          placeholder={
            state.pending || fullAutoRunning
              ? activeSubmitMode === "steer"
                ? "Steer the current turn…"
                : "Queue a follow-up…"
              : `Message ${capabilities?.displayName ?? (state.selectedHarness === "codex" ? "Codex" : "Claude")}…`
          }
          ariaLabel={state.pending || fullAutoRunning ? `${submitLabel} a ${capabilities?.displayName ?? "provider"} message` : `Message ${capabilities?.displayName ?? "provider"}`}
          disabled={false}
          onChange={(value) => dispatch(report, "DesktopInputChanged", value)}
          onSubmit={submit}
        />
        <DesktopComposerBar>
        <DesktopComposerButton
          data-en-key="shell-composer-discovery"
          kind="action"
          aria-expanded={discoveryOpen}
          onClick={() => setDiscoveryOpen(open => !open)}
          aria-label="Add context or command"
          title="Add context or command"
        >
          <CommandIcon data-icon-name="Command" aria-hidden="true" />
        </DesktopComposerButton>
        {imageSupported ? <DesktopComposerButton
          data-en-key="shell-attach-image"
          kind="action"
          disabled={attachmentDisabled}
          onClick={() => dispatch(report, "DesktopComposerImagePickRequested")}
          aria-label={attachmentLabel}
          title={attachmentLabel}
        >
          <ImagePlus data-icon-name={composerIconNames.attach} aria-hidden="true" />
        </DesktopComposerButton> : null}
        <DesktopComposerButton
          data-en-key="shell-provider-select"
          kind="action"
          disabled={!canSwitchHarness}
          onClick={() => dispatch(report, "DesktopHarnessSelected", otherHarness)}
          aria-label={`Provider: ${capabilities?.displayName ?? state.selectedHarness}. ${canSwitchHarness ? `Switch to ${otherCapabilities?.displayName ?? otherHarness}` : "No other admitted provider is available"}`}
          title={`Provider: ${capabilities?.displayName ?? state.selectedHarness}`}
        >
          {capabilities?.displayName ?? (state.selectedHarness === "codex" ? "Codex" : "Claude")}
        </DesktopComposerButton>
        <DesktopComposerButton
          data-en-key="shell-model-select"
          kind="action"
          disabled={state.pending || fullAutoRunning || visibleModels.length < 2 || !capabilityAdmitted}
          onClick={() => dispatch(report, "DesktopModelSelected", nextModel)}
          aria-label={`Model: ${selectedModel}`}
          title={`Model: ${selectedModel}`}
        >
          {selectedModel}
        </DesktopComposerButton>
        <div className="oa-react-composer-secondary-controls" data-open={secondaryControlsOpen ? "true" : "false"}>
          <button type="button" className="oa-react-composer-secondary-trigger"
            aria-expanded={secondaryControlsOpen}
            aria-label="More composer controls" title="More composer controls"
            onClick={() => setSecondaryControlsOpen(open => !open)}>
            <Ellipsis aria-hidden="true" />
          </button>
          <div className="oa-react-composer-secondary-controls-content">
        {capabilities !== null && capabilities.reasoningEfforts.length > 0 ? <DesktopComposerButton
          data-en-key="shell-reasoning-select"
          kind="action"
          disabled={state.pending || fullAutoRunning}
          onClick={() => {
            const index = Math.max(0, capabilities.reasoningEfforts.indexOf(state.codexReasoningEffort));
            dispatch(report, "DesktopCodexReasoningSelected", capabilities.reasoningEfforts[(index + 1) % capabilities.reasoningEfforts.length] ?? state.codexReasoningEffort);
          }}
          aria-label={`Reasoning effort: ${state.codexReasoningEffort}`}
          title={`Reasoning: ${state.codexReasoningEffort}`}
        >
          {state.codexReasoningEffort}
        </DesktopComposerButton> : null}
        {capabilities !== null && capabilities.permissionModes.includes("plan_only") && state.activeThreadId !== null ? <DesktopComposerButton
          data-en-key="shell-permission-mode"
          kind="action"
          disabled={state.pending || fullAutoRunning}
          onClick={() => dispatch(report, "DesktopPermissionModeSelected", (state.permissionModeByThread[state.activeThreadId!] ?? "owner_full") === "owner_full" ? "plan_only" : "owner_full")}
          aria-label={(state.permissionModeByThread[state.activeThreadId] ?? "owner_full") === "owner_full" ? "Full tools. Switch to plan only" : "Plan only. Switch to full tools"}
          title="Permission mode"
        >
          {(state.permissionModeByThread[state.activeThreadId] ?? "owner_full") === "owner_full" ? "Full tools" : "Plan only"}
        </DesktopComposerButton> : null}
        {(capabilities?.fullAuto ?? true) ? <DesktopComposerButton
          data-en-key="shell-full-auto-toggle"
          kind="toggle"
          // FA-H1 #8874: the pressed state and label read the ACTIVE thread's
          // durable-truth entry, so a thread main resumed after a restart
          // shows ON and a single click honestly means "turn it off".
          aria-pressed={activeFullAutoEnabled(state)}
          onClick={() => dispatch(report, "DesktopFullAutoToggled")}
          aria-label={activeFullAutoEnabled(state) ? "Turn off Full Auto" : "Turn on Full Auto"}
          title="Full Auto: Codex looks at this repo and keeps working, turn after turn, until you turn it off"
        >
          <Zap data-icon-name={composerIconNames.fullAuto} aria-hidden="true" />
          Full Auto
        </DesktopComposerButton> : null}
          </div>
        </div>
        {state.pending && ((capabilities?.steerTurn ?? true) || (capabilities?.queueFollowup ?? true)) ? (
          <Button
            className="oa-react-submit-mode-toggle"
            type="button"
            variant="secondary"
            size="sm"
            aria-label={canTogglePendingMode
              ? `${pendingAction.label}. Switch to ${alternatePendingAction.label.toLowerCase()}`
              : `${pendingAction.label}. ${alternatePendingAction.consequence}`}
            title={canTogglePendingMode
              ? `Click to switch to ${alternatePendingAction.label.toLowerCase()}`
              : alternatePendingAction.consequence}
            disabled={!canTogglePendingMode}
            onClick={() => dispatch(report, "DesktopPendingSubmitModeSelected", alternatePendingMode)}
          >
            {pendingAction.label}
          </Button>
        ) : null}
        <span className="oa-react-composer-spacer" />
        {!state.pending && (!lane.available || !capabilityAdmitted) ? (
          <Badge
            className="oa-react-composer-status"
            variant="outline"
            role="status"
            aria-live="polite"
            data-codex-status={lane.reason === CODEX_CHIP_REASON_VERIFYING ? "checking" : "unavailable"}
          >
            <span className="oa-react-composer-status-dot" aria-hidden="true" />
            {lane.reason === CODEX_CHIP_REASON_VERIFYING ? "Checking Codex…" : capabilities?.reason ?? lane.reason ?? `${capabilities?.displayName ?? "Provider"} unavailable`}
          </Badge>
        ) : null}
        {!state.pending && fullAutoRunning ? (
          <Badge
            className="oa-react-composer-status"
            variant="outline"
            role="status"
            aria-live="polite"
            data-full-auto-status="running"
          >
            <span className="oa-react-composer-status-dot" aria-hidden="true" />
            Full Auto running…
          </Badge>
        ) : null}
        {(state.pending || fullAutoRunning) && (capabilities?.interrupt ?? true) ? (
          <DesktopComposerButton
            kind="stop"
            onClick={() => dispatch(report, "DesktopTurnInterrupted")}
            aria-label="Stop current turn"
            title="Stop"
          >
            <Square data-icon-name={composerIconNames.stop} aria-hidden="true" />
            <span className="sr-only">Stop</span>
          </DesktopComposerButton>
        ) : null}
        <DesktopComposerButton
          kind="submit"
          type="button"
          disabled={!canSubmit}
          onClick={() => submit(editorRef.current?.readValue() ?? state.input)}
          aria-label={submitLabel} title={submitLabel}>
          <ArrowUp data-icon-name={composerIconNames.submit} aria-hidden="true" />
          <span className="sr-only">{submitLabel}</span>
        </DesktopComposerButton>
      </DesktopComposerBar>
      </DesktopComposerInput>
    </DesktopComposerFrame>
  );
};

const decisionTitle = (kind: ReturnType<typeof questionKind>, providerName = "Agent"): string =>
  kind === "tool_approval"
    ? "Tool approval"
    : kind === "plan_review"
      ? "Review plan"
      : `${providerName} needs your input`;

const decisionIntent = (kind: ReturnType<typeof questionKind>, label: string): string | null => {
  if (kind === "tool_approval")
    return label === "Approve"
      ? "DesktopApprovalApproved"
      : label === "Deny"
        ? "DesktopApprovalDenied"
        : null;
  if (kind === "plan_review")
    return label === "Accept"
      ? "DesktopPlanAccepted"
      : label === "Request changes"
        ? "DesktopPlanChangesRequested"
        : label === "Replan"
          ? "DesktopPlanReplanRequested"
          : null;
  return null;
};

/**
 * Plan review's three canonical outcomes, in the exact label vocabulary both
 * decision backends require verbatim (`selectRuntimeDecision` in shell.ts,
 * and the Runtime Gateway `RuntimeInteraction` decision envelope in
 * renderer/runtime-interactions.ts, which rejects anything else).
 */
const planReviewActionLabels = ["Accept", "Request changes", "Replan"] as const;

/**
 * T9 #8866: `tool_approval` and `plan_review` are both structurally a single
 * question with a small fixed set of named outcomes (Approve/Deny, or
 * Accept/Request changes/Replan) — the same shape `DesktopApprovalCard`
 * already renders for read-only history rows
 * (`packages/ui/src/workbench/dispatch.tsx` `case "approval"`). Routing the
 * LIVE interactive card through the same shared component means both the
 * past-tense and present-tense presentations of an approval decision use
 * identical Autopilot styling. `provider_question` stays on the generic
 * fieldset renderer below unchanged: it can carry multiple questions with
 * per-option multi-select and a "Submit choices" footer, a shape
 * `DesktopApprovalCard` does not (and should not) try to fit.
 */
const isApprovalCardKind = (
  kind: ReturnType<typeof questionKind>,
): kind is "tool_approval" | "plan_review" => kind === "tool_approval" || kind === "plan_review";

export const DecisionSurface = ({
  state,
  report,
}: {
  readonly state: DesktopShellState;
  readonly report: IntentReporter;
}): ReactElement | null => {
  const note = activeQuestionNote(state);
  const card = note?.question;
  if (note === null || card === undefined) return null;
  const interaction: QuestionCardInteraction | undefined = state.questionCards[card.questionRef];
  const kind = questionKind(note);
  const providerName = capabilityForHarness(state)?.displayName ??
    (state.selectedHarness === "codex" ? "Codex" : "Claude");
  const title = decisionTitle(kind, providerName);
  const answered = interaction?.answered === true;
  const submitting = interaction?.submitting === true;
  const unavailable = !state.questionAnswerHostAvailable;
  const anyMulti = card.questions.some((question) => question.multiSelect);
  const ready = interaction !== undefined && questionAnswersReady(card, interaction);
  return (
    <section
      className="oa-react-decision"
      aria-labelledby={`decision-title-${card.questionRef}`}
      aria-describedby={`decision-description-${card.questionRef}`}
      data-decision-kind={kind}
    >
        <header className="oa-react-decision-header">
          <h2 id={`decision-title-${card.questionRef}`}>{title}</h2>
          <p id={`decision-description-${card.questionRef}`}>
            {unavailable
              ? "This request is read-only because the answer bridge is unavailable."
              : answered
                ? "Submitted. Waiting for the runtime to confirm the outcome."
                : submitting
                  ? "Submitting this decision…"
                  : kind === "provider_question"
                    ? "Waiting for your answer. Choose an option or write another answer below."
                    : kind === "plan_review"
                      ? "Plan ready. Accept it, request changes, or ask the agent to replan."
                      : "Review the request below. It remains pending until you decide."}
          </p>
        </header>
        {isApprovalCardKind(kind) ? (() => {
          // Both approval kinds emit exactly one question in practice (the
          // command/patch/plan under review); the fixed action vocabulary
          // above is per-kind, not per-option, so only the first question's
          // text/header feed the card.
          const question = card.questions[0];
          // The single label the user picked, recorded the instant a button
          // is clicked (`withQuestionSelection`, before the reducer even
          // starts the submit round trip) and left in place across an
          // "answer_refused" rejection so retry can read it back — see
          // shell.ts `withQuestionAnswerRejected`. Only treat it as a
          // RESOLVED outcome once the runtime actually confirmed
          // (`answered`); while merely `submitting` the card still shows the
          // interactive controls (a same-instant double-click races the
          // reducer's own no-op guard in `withQuestionSelection`, never a
          // duplicate submission).
          const chosenLabel = interaction?.selections[0]?.[0];
          const decision: DesktopApprovalDecision = !answered
            ? "pending"
            : kind === "tool_approval"
              ? (chosenLabel === "Deny" ? "denied" : "approved")
              : (chosenLabel === "Accept" ? "approved" : "denied");
          const decisionLabel = answered && kind === "plan_review" ? chosenLabel : undefined;
          const canDecide = !unavailable && !answered;
          const onDecision = canDecide && kind === "tool_approval"
            ? (next: Exclude<DesktopApprovalDecision, "pending">) =>
                dispatch(
                  report,
                  next === "approved" ? "DesktopApprovalApproved" : "DesktopApprovalDenied",
                  card.questionRef,
                )
            : undefined;
          const actions: ReadonlyArray<DesktopApprovalAction> | undefined = canDecide && kind === "plan_review"
            ? planReviewActionLabels.map((label): DesktopApprovalAction => ({
                key: label,
                label,
                primary: label === "Accept",
                onSelect: () => dispatch(
                  report,
                  label === "Accept"
                    ? "DesktopPlanAccepted"
                    : label === "Request changes"
                      ? "DesktopPlanChangesRequested"
                      : "DesktopPlanReplanRequested",
                  card.questionRef,
                ),
              }))
            : undefined;
          return (
            <DesktopApprovalCard
              actions={actions}
              decision={decision}
              decisionLabel={decisionLabel}
              description={question?.header ?? title}
              itemKey={`decision-${card.questionRef}`}
              onDecision={onDecision}
              resource={question?.question ?? ""}
              title={title}
            />
          );
        })() : (
          <div className="oa-react-decision-questions">
            {card.questions.map((question, questionIndex) => (
              <fieldset
                key={question.questionRef ?? `${card.questionRef}:${questionIndex}`}
                disabled={unavailable || answered || submitting}
              >
                <legend>
                  <span>{question.header}</span>
                  {question.question}
                </legend>
                <div className="oa-react-decision-options">
                  {question.options.map((option) => {
                    const selected =
                      interaction?.selections[questionIndex]?.includes(option.label) === true;
                    return (
                      <Button
                        key={option.optionRef ?? option.label}
                        type="button"
                        variant={selected ? "secondary" : "outline"}
                        aria-pressed={selected}
                        onClick={() => {
                          const intent = decisionIntent(kind, option.label);
                          if (intent === null)
                            dispatch(report, "DesktopQuestionOptionSelected", {
                              questionRef: card.questionRef,
                              questionIndex,
                              label: option.label,
                            });
                          else dispatch(report, intent, card.questionRef);
                        }}
                      >
                        <span>{option.label}</span>
                        {option.description === undefined ? null : (
                          <small>{option.description}</small>
                        )}
                      </Button>
                    );
                  })}
                </div>
                <label className="oa-react-decision-other">
                  <span>Other</span>
                  <Textarea
                    rows={2}
                    value={interaction?.texts?.[questionIndex] ?? ""}
                    placeholder="Type another answer"
                    aria-label={`Other answer for ${question.question}`}
                    onChange={(event) => dispatch(report, "DesktopQuestionTextChanged", {
                      form: card.questionRef,
                      field: String(questionIndex),
                      value: event.currentTarget.value,
                    })}
                  />
                </label>
              </fieldset>
            ))}
          </div>
        )}
        {interaction?.failure === "answer_refused" ? (
          <p className="oa-react-decision-failure" role="alert">
            The runtime did not accept that decision. Review it and try again.
          </p>
        ) : null}
        {kind === "provider_question" && !answered ? (
          <div className="oa-react-decision-footer">
            <Button
              type="button"
              disabled={unavailable || submitting || !ready}
              onClick={() => dispatch(report, "DesktopQuestionSubmitted", card.questionRef)}
            >
              {anyMulti ? "Submit choices" : "Submit answer"}
            </Button>
          </div>
        ) : null}
    </section>
  );
};
