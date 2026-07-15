import { Button } from "#components/ui/button";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
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
import { Textarea } from "#components/ui/textarea";
import {
  ComponentValueBinding,
  IntentRef,
  type IntentError,
  type IntentReporter,
  type JsonPayload,
} from "@effect-native/core";
import { Effect } from "@effect-native/core/effect";
import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactElement,
} from "react";

import {
  desktopCommandRegistry,
  formatCommandChord,
  type DesktopCommand,
} from "./command-registry.ts";
import type { DesktopNoteEntry, DesktopShellState, QuestionCardInteraction } from "./shell.ts";

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(
    report(payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload) as Effect.Effect<void, IntentError>,
  ).catch(() => {});
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

export const ReactCommandPalette = ({
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
      desktopCommandRegistry.filter(
        (command) =>
          normalized === "" ||
          `${command.label} ${command.id}`.toLocaleLowerCase().includes(normalized),
      ),
    [normalized],
  );
  useEffect(() => {
    if (!state.commandPaletteOpen) setQuery("");
  }, [state.commandPaletteOpen]);
  return (
    <CommandDialog
      open={state.commandPaletteOpen}
      onOpenChange={(open) => {
        if (!open) dispatch(report, "DesktopCommandPaletteDismissed");
      }}
      title="Commands"
      description="Search the canonical Desktop command registry"
    >
      <Command shouldFilter={false} label="Desktop commands">
        <CommandInput
          value={query}
          onValueChange={setQuery}
          placeholder="Type a command…"
          autoFocus
        />
        <CommandList>
          <CommandEmpty>No matching commands.</CommandEmpty>
          <CommandGroup heading="Commands">
            {commands.map((command) => {
              const available = commandAvailable(command, state);
              const chord = formatCommandChord(command.chords, state.host.includes("darwin"));
              return (
                <CommandItem
                  key={command.id}
                  value={command.id}
                  disabled={!available}
                  aria-disabled={!available}
                  onSelect={() => {
                    if (!available) return;
                    dispatch(report, command.intentName, command.payload);
                    dispatch(report, "DesktopCommandPaletteDismissed");
                  }}
                >
                  <span>{command.label}</span>
                  {chord === null ? null : <CommandShortcut>{chord}</CommandShortcut>}
                </CommandItem>
              );
            })}
          </CommandGroup>
        </CommandList>
        <span className="oa-react-sr-only" role="status" aria-live="polite">
          {commands.length} commands
        </span>
      </Command>
    </CommandDialog>
  );
};

export const ReactComposer = ({
  state,
  report,
}: {
  readonly state: DesktopShellState;
  readonly report: IntentReporter;
}): ReactElement => {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const composingRef = useRef(false);
  const lastSubmitRef = useRef<Readonly<{ value: string; at: number }> | null>(null);
  const sessionKey = state.activeThreadId ?? state.history.page?.selectedThreadRef ?? "new";
  const lane = state.harnessLanes[state.selectedHarness];
  const canSubmit =
    state.activeThreadId !== null && state.input.trim() !== "" && (state.pending || lane.available);
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
  const submit = (): void => {
    if (!canSubmit) return;
    const now = Date.now();
    if (lastSubmitRef.current?.value === state.input && now - lastSubmitRef.current.at < 350)
      return;
    lastSubmitRef.current = { value: state.input, at: now };
    dispatch(report, submitIntent, state.input);
  };
  useEffect(() => {
    const active = textareaRef.current?.ownerDocument.activeElement;
    if (active === null || active === textareaRef.current?.ownerDocument.body)
      textareaRef.current?.focus();
  }, [sessionKey]);
  useLayoutEffect(() => {
    const textarea = textareaRef.current;
    if (textarea === null) return;
    textarea.style.height = "auto";
    const next = Math.min(180, Math.max(64, textarea.scrollHeight));
    textarea.style.height = `${next}px`;
    textarea.style.overflowY = textarea.scrollHeight > 180 ? "auto" : "hidden";
  }, [state.input]);
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>): void => {
    if (event.key !== "Enter" || event.shiftKey) return;
    if (composingRef.current || event.nativeEvent.isComposing) return;
    event.preventDefault();
    submit();
  };
  return (
    <section className="oa-react-composer" aria-label="Message composer">
      <Textarea
        ref={textareaRef}
        value={state.input}
        rows={2}
        placeholder={
          state.pending
            ? state.pendingSubmitMode === "steer"
              ? "Steer the current turn…"
              : "Queue a follow-up…"
            : "Message Codex…"
        }
        aria-label={state.pending ? `${submitLabel} a Codex message` : "Message Codex"}
        onInput={(event) => dispatch(report, "DesktopInputChanged", event.currentTarget.value)}
        onCompositionStart={() => {
          composingRef.current = true;
        }}
        onCompositionEnd={() => {
          composingRef.current = false;
        }}
        onKeyDown={onKeyDown}
      />
      <div className="oa-react-composer-bar">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => dispatch(report, "DesktopCommandPaletteToggled")}
        >
          Commands
        </Button>
        {state.pending ? (
          <div className="oa-react-submit-mode" aria-label="Pending message behavior">
            <Button
              type="button"
              variant={state.pendingSubmitMode === "steer" ? "secondary" : "ghost"}
              size="sm"
              aria-pressed={state.pendingSubmitMode === "steer"}
              onClick={() => dispatch(report, "DesktopPendingSubmitModeSelected", "steer")}
            >
              Steer
            </Button>
            <Button
              type="button"
              variant={state.pendingSubmitMode === "queue" ? "secondary" : "ghost"}
              size="sm"
              aria-pressed={state.pendingSubmitMode === "queue"}
              onClick={() => dispatch(report, "DesktopPendingSubmitModeSelected", "queue")}
            >
              Queue
            </Button>
          </div>
        ) : null}
        <span className="oa-react-composer-spacer" />
        {!state.pending && !lane.available ? (
          <span className="oa-react-composer-status" role="status">
            {lane.reason ?? "Codex is unavailable"}
          </span>
        ) : null}
        {state.pending ? (
          <Button
            type="button"
            variant="outline"
            onClick={() => dispatch(report, "DesktopTurnInterrupted")}
          >
            Stop
          </Button>
        ) : null}
        <Button type="button" disabled={!canSubmit} onClick={submit}>
          {submitLabel}
        </Button>
      </div>
      <span className="oa-react-composer-hint">
        Enter to {submitLabel.toLocaleLowerCase()} · Shift+Enter for a new line
      </span>
    </section>
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
