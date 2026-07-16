import { Button } from "#components/ui/button";
import { Badge } from "#components/ui/badge";
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
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "#components/ui/dialog";
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
  DesktopComposerBar,
  DesktopComposerButton,
  DesktopComposerFrame,
  DesktopComposerInput,
} from "@openagentsinc/ui/desktop-workbench";
import {
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  Command as CommandIcon,
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
import { formatRelativeTimestamp, type DesktopNoteEntry, type DesktopShellState, type QuestionCardInteraction } from "./shell.ts";
import {
  LexicalComposerEditor,
  type LexicalComposerEditorHandle,
} from "./lexical-composer-editor.tsx";

const composerIconNames = {
  commands: "Command",
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
  if (command.id.startsWith("interaction.question"))
    return kind === "provider_question" && state.questionAnswerHostAvailable;
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

export const ReactComposer = ({
  state,
  report,
}: {
  readonly state: DesktopShellState;
  readonly report: IntentReporter;
}): ReactElement => {
  const editorRef = useRef<LexicalComposerEditorHandle>(null);
  const [dragActive, setDragActive] = useState(false);
  const lastSubmitRef = useRef<Readonly<{ value: string; at: number }> | null>(null);
  const sessionKey = state.activeThreadId ?? state.history.page?.selectedThreadRef ?? "new";
  const lane = state.harnessLanes[state.selectedHarness];
  const pendingAction = composerActionPresentation(state.composerAdmission, state.pendingSubmitMode);
  const hasText = state.input.trim() !== "";
  const canSubmit = state.pending
    ? state.activeThreadId !== null && hasText && pendingAction.enabled
    : lane.available && (hasText || state.composerImages.length > 0);
  const atImageLimit = !canAttachMoreImages(state.composerImages);
  const attachmentDisabled = state.pending || atImageLimit;
  const attachmentLabel = state.pending
    ? "Attach images after the current turn finishes"
    : atImageLimit
      ? `Image limit reached (${COMPOSER_IMAGE_COUNT_LIMIT} max)`
      : "Attach images";
  useEffect(() => {
    if (attachmentDisabled) setDragActive(false);
  }, [attachmentDisabled]);
  const submitIntent = state.pending
    ? state.pendingSubmitMode === "steer"
      ? "DesktopSteerCurrentRequested"
      : "DesktopQueueNextRequested"
    : "DesktopNoteSubmitted";
  const submitLabel = state.pending
    ? state.pendingSubmitMode === "steer"
      ? "Steer"
      : "Queue"
    : "Send";
  const submit = (editorValue = state.input): void => {
    const nextHasText = editorValue.trim() !== "";
    const submissionKey = nextHasText ? editorValue : `images:${state.composerImages.length}`;
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
        <ol className="oa-react-composer-queue" aria-label="Queued turns">
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
      <DesktopComposerInput>
        <LexicalComposerEditor
          editorRef={editorRef}
          value={state.input}
          placeholder={
            state.pending
              ? state.pendingSubmitMode === "steer"
                ? "Steer the current turn…"
                : "Queue a follow-up…"
              : "Message Codex…"
          }
          ariaLabel={state.pending ? `${submitLabel} a Codex message` : "Message Codex"}
          disabled={false}
          onChange={(value) => dispatch(report, "DesktopInputChanged", value)}
          onSubmit={submit}
        />
        <DesktopComposerBar>
        <DesktopComposerButton
          data-en-key="shell-attach-image"
          kind="action"
          disabled={attachmentDisabled}
          onClick={() => dispatch(report, "DesktopComposerImagePickRequested")}
          aria-label={attachmentLabel}
          title={attachmentLabel}
        >
          <ImagePlus data-icon-name={composerIconNames.attach} aria-hidden="true" />
        </DesktopComposerButton>
        <DesktopComposerButton
          kind="action"
          onClick={() => dispatch(report, "DesktopCommandPaletteToggled")}
          aria-label="Open commands"
          title="Commands"
        >
          <CommandIcon data-icon-name={composerIconNames.commands} aria-hidden="true" />
        </DesktopComposerButton>
        <DesktopComposerButton
          data-en-key="shell-full-auto-toggle"
          kind="toggle"
          aria-pressed={state.fullAuto}
          onClick={() => dispatch(report, "DesktopFullAutoToggled")}
          aria-label={state.fullAuto ? "Turn off Full Auto" : "Turn on Full Auto"}
          title="Full Auto: Codex looks at this repo and keeps working, turn after turn, until you turn it off"
        >
          <Zap data-icon-name={composerIconNames.fullAuto} aria-hidden="true" />
          Full Auto
        </DesktopComposerButton>
        {state.pending ? (
          <div className="oa-react-submit-mode" role="radiogroup" aria-label="Pending message behavior">
            <Button
              type="button"
              variant={state.pendingSubmitMode === "steer" ? "secondary" : "ghost"}
              size="sm"
              role="radio"
              aria-label="Steer now"
              aria-pressed={state.pendingSubmitMode === "steer"}
              aria-checked={state.pendingSubmitMode === "steer"}
              disabled={!composerActionPresentation(state.composerAdmission, "steer").enabled}
              onClick={() => dispatch(report, "DesktopPendingSubmitModeSelected", "steer")}
            >
              Steer now
            </Button>
            <Button
              type="button"
              variant={state.pendingSubmitMode === "queue" ? "secondary" : "ghost"}
              size="sm"
              role="radio"
              aria-label="Queue next"
              aria-pressed={state.pendingSubmitMode === "queue"}
              aria-checked={state.pendingSubmitMode === "queue"}
              disabled={!composerActionPresentation(state.composerAdmission, "queue").enabled}
              onClick={() => dispatch(report, "DesktopPendingSubmitModeSelected", "queue")}
            >
              Queue next
            </Button>
          </div>
        ) : null}
        <span className="oa-react-composer-spacer" />
        {!state.pending && !lane.available ? (
          <Badge
            className="oa-react-composer-status"
            variant="outline"
            role="status"
            aria-live="polite"
            data-codex-status={lane.reason === CODEX_CHIP_REASON_VERIFYING ? "checking" : "unavailable"}
          >
            <span className="oa-react-composer-status-dot" aria-hidden="true" />
            {lane.reason === CODEX_CHIP_REASON_VERIFYING ? "Checking Codex…" : lane.reason ?? "Codex unavailable"}
          </Badge>
        ) : null}
        {state.pending ? (
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
      {state.pending ? (
        <p className="oa-react-composer-consequence" role="status" aria-live="polite">
          <strong>{pendingAction.label}</strong>
          <span>{state.composerQueueEditingRef === null ? pendingAction.consequence : "Editing a durable queued turn; its position is preserved."}</span>
        </p>
      ) : null}
      </DesktopComposerInput>
    </DesktopComposerFrame>
  );
};

const decisionTitle = (kind: ReturnType<typeof questionKind>): string =>
  kind === "tool_approval"
    ? "Tool approval"
    : kind === "plan_review"
      ? "Review plan"
      : "Codex needs your input";

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

export const DecisionSurface = ({
  state,
  report,
}: {
  readonly state: DesktopShellState;
  readonly report: IntentReporter;
}): ReactElement | null => {
  const note = activeQuestionNote(state);
  const card = note?.question;
  const [dismissedRef, setDismissedRef] = useState<string | null>(null);
  useEffect(() => {
    if (card !== undefined && dismissedRef !== card.questionRef) setDismissedRef(null);
  }, [card?.questionRef]);
  if (note === null || card === undefined) return null;
  const interaction: QuestionCardInteraction | undefined = state.questionCards[card.questionRef];
  const kind = questionKind(note);
  const answered = interaction?.answered === true;
  const submitting = interaction?.submitting === true;
  const unavailable = !state.questionAnswerHostAvailable;
  const anyMulti = card.questions.some((question) => question.multiSelect);
  const ready = card.questions.every(
    (question, index) => (interaction?.selections[index]?.length ?? 0) > 0,
  );
  return (
    <Dialog
      open={dismissedRef !== card.questionRef}
      onOpenChange={(open) => {
        if (!open) setDismissedRef(card.questionRef);
      }}
    >
      <DialogContent
        className="oa-react-decision"
        aria-describedby={`decision-description-${card.questionRef}`}
      >
        <DialogHeader>
          <DialogTitle>{decisionTitle(kind)}</DialogTitle>
          <DialogDescription id={`decision-description-${card.questionRef}`}>
            {unavailable
              ? "This request is read-only because the answer bridge is unavailable."
              : answered
                ? "Submitted. Waiting for the runtime to confirm the outcome."
                : submitting
                  ? "Submitting this decision…"
                  : "Choose an option. Closing this dialog does not approve or deny anything."}
          </DialogDescription>
        </DialogHeader>
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
            </fieldset>
          ))}
        </div>
        {interaction?.failure === "answer_refused" ? (
          <p className="oa-react-decision-failure" role="alert">
            The runtime did not accept that decision. Review it and try again.
          </p>
        ) : null}
        {anyMulti && kind === "provider_question" && !answered ? (
          <DialogFooter>
            <Button
              type="button"
              disabled={unavailable || submitting || !ready}
              onClick={() => dispatch(report, "DesktopQuestionSubmitted", card.questionRef)}
            >
              Submit choices
            </Button>
          </DialogFooter>
        ) : null}
      </DialogContent>
    </Dialog>
  );
};
