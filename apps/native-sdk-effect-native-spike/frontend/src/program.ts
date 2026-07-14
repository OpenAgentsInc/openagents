import {
  Badge,
  Card,
  ComponentValueBinding,
  defineIntent,
  IconButton,
  IntentRef,
  makeIntentRegistry,
  makeViewProgramFromState,
  Spacer,
  Stack,
  Text,
  TextField,
  Transcript,
  type TranscriptMessage,
  type View,
} from "@effect-native/core";
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect";

export const fixtureSessions = [
  { ref: "session.parity", title: "Native parity pass" },
  { ref: "session.renderer", title: "Renderer boundary" },
  { ref: "session.audit", title: "SDK adoption audit" },
] as const;

export type Workspace = "chat" | "home" | "settings";

export interface SpikeMessage {
  readonly key: string;
  readonly role: "user" | "assistant" | "system";
  readonly text: string;
  readonly timestamp: string;
}

export interface SpikeState {
  readonly workspace: Workspace;
  readonly selectedSessionRef: string | null;
  readonly input: string;
  readonly messages: ReadonlyArray<SpikeMessage>;
  readonly pending: boolean;
  readonly revision: number;
}

export const DesktopInputChanged = defineIntent("DesktopInputChanged", Schema.String);
export const DesktopNoteSubmitted = defineIntent("DesktopNoteSubmitted", Schema.NullOr(Schema.String));
export const DesktopTurnInterrupted = defineIntent("DesktopTurnInterrupted", Schema.Null);
export const DesktopNewChat = defineIntent("DesktopNewChat", Schema.Null);
export const DesktopWorkspaceSelected = defineIntent("DesktopWorkspaceSelected", Schema.Literals(["chat", "home"]));
export const DesktopSettingsToggled = defineIntent("DesktopSettingsToggled", Schema.Null);
export const DesktopChatSelected = defineIntent("DesktopChatSelected", Schema.String);
export const spikeIntents = [
  DesktopInputChanged,
  DesktopNoteSubmitted,
  DesktopTurnInterrupted,
  DesktopNewChat,
  DesktopWorkspaceSelected,
  DesktopSettingsToggled,
  DesktopChatSelected,
] as const;

const initialMessages: ReadonlyArray<SpikeMessage> = [
  {
    key: "fixture-user",
    role: "user",
    text: "Bring the Native SDK spike one step closer to the real desktop app.",
    timestamp: "10:42 AM",
  },
  {
    key: "fixture-assistant",
    role: "assistant",
    text: "I’ll prove the session rail, transcript, composer, and a bounded Effect-authoritative bridge.",
    timestamp: "10:42 AM",
  },
];

export const initialSpikeState = (): SpikeState => ({
  workspace: "chat",
  selectedSessionRef: fixtureSessions[0].ref,
  input: "",
  messages: initialMessages,
  pending: false,
  revision: 1,
});

const transcriptMessage = (message: SpikeMessage): TranscriptMessage => ({
  key: message.key,
  role: message.role,
  ...(message.role === "assistant" ? {} : { senderLabel: message.role === "user" ? "YOU" : "SYSTEM" }),
  timestamp: message.timestamp,
  body: [Text({
    key: `${message.key}-body`,
    content: message.text,
    variant: "body",
    color: message.role === "system" ? "textMuted" : "textPrimary",
  })],
});

const composer = (state: SpikeState): View => Card({
  key: "spike-composer",
  padding: "2",
  radius: "xl",
  style: {
    width: "full",
    maxWidth: "2xl",
    alignSelf: "center",
    borderColor: "border",
    borderWidth: 1,
    marginBottom: "4",
    surface: "glass",
  },
}, [
  TextField({
    key: "spike-input",
    value: state.input,
    multiline: true,
    placeholder: state.pending ? "Turn running…" : "Message",
    clearOnSubmit: true,
    onChange: IntentRef("DesktopInputChanged", ComponentValueBinding()),
    onSubmit: IntentRef("DesktopNoteSubmitted", ComponentValueBinding()),
    style: { width: "full", minHeight: "2xs" },
    a11y: { label: "Message" },
  }),
  Stack({ key: "spike-composer-bar", direction: "row", gap: "1", align: "center", style: { width: "full" } }, [
    Text({ key: "spike-engine", content: "Codex", variant: "label", color: "textMuted" }),
    Spacer({ key: "spike-composer-fill", flex: true }),
    IconButton({
      key: state.pending ? "spike-stop" : "spike-send",
      icon: state.pending ? "Stop" : "ArrowUp",
      accessibilityLabel: state.pending ? "Stop turn" : "Send message",
      onPress: state.pending
        ? IntentRef("DesktopTurnInterrupted")
        : IntentRef("DesktopNoteSubmitted"),
      style: state.input.trim() === "" || state.pending
        ? { backgroundColor: "surfaceRaised", color: "textMuted", borderRadius: "full" }
        : { backgroundColor: "accent", color: "textInverse", borderRadius: "full" },
    }),
  ]),
]);

const chatView = (state: SpikeState): View => Stack({
  key: "spike-chat",
  direction: "column",
  gap: "3",
  padding: "4",
  style: { width: "full", minHeight: 0, flex: 1 },
}, [
  Stack({ key: "spike-chat-header", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
    Stack({ key: "spike-chat-heading", direction: "column", gap: "0.5" }, [
      Text({
        key: "spike-chat-title",
        content: fixtureSessions.find((session) => session.ref === state.selectedSessionRef)?.title ?? "New chat",
        variant: "title",
        color: "textPrimary",
        weight: "semibold",
      }),
      Text({ key: "spike-chat-status", content: state.pending ? "Codex is working" : "Local fixture · no provider call", variant: "caption", color: "textMuted" }),
    ]),
    Spacer({ key: "spike-chat-header-fill", flex: true }),
    Badge({ key: "spike-proof-badge", label: "HYBRID FIXTURE", tone: "info", variant: "soft", size: "sm" }),
  ]),
  Transcript({
    key: "spike-transcript",
    pinToEnd: true,
    messages: state.messages.map(transcriptMessage),
    style: { width: "full", maxWidth: "2xl", alignSelf: "center", flex: 1, minHeight: 0, paddingLeft: "4", paddingRight: "4", gap: "3" },
  }),
  composer(state),
]);

const secondaryView = (state: SpikeState): View => Stack({
  key: `spike-${state.workspace}`,
  direction: "column",
  gap: "2",
  padding: "6",
  style: { width: "full", minHeight: 0, flex: 1 },
}, [
  Text({ key: "spike-secondary-title", content: state.workspace === "home" ? "Workspace" : "Settings", variant: "heading", color: "textPrimary", weight: "bold" }),
  Text({
    key: "spike-secondary-copy",
    content: "This parity pass only implements the real app’s bounded MVP chat surface. Other workspaces remain explicit fixtures.",
    variant: "body",
    color: "textMuted",
  }),
]);

export const spikeView = (state: SpikeState): View =>
  state.workspace === "chat" ? chatView(state) : secondaryView(state);

const nextRevision = (state: SpikeState): number => state.revision + 1;

export const makeSpikeRuntime = (restoredState: SpikeState = initialSpikeState()) => Effect.gen(function* () {
  const state = yield* SubscriptionRef.make(restoredState);
  const registry = yield* makeIntentRegistry(spikeIntents, {
    DesktopInputChanged: (value: string) => SubscriptionRef.update(state, (current) => ({ ...current, input: value.slice(0, 4_000) })),
    DesktopNoteSubmitted: (value: string | null) => SubscriptionRef.update(state, (current) => {
      const text = (value ?? "").trim() || current.input.trim();
      if (text === "") return current;
      return {
        ...current,
        input: "",
        pending: true,
        revision: nextRevision(current),
        messages: [...current.messages, {
          key: `fixture-user-${current.revision + 1}`,
          role: "user" as const,
          text: text.slice(0, 4_000),
          timestamp: "now",
        }],
      };
    }),
    DesktopTurnInterrupted: () => SubscriptionRef.update(state, (current) => current.pending
      ? { ...current, pending: false, revision: nextRevision(current) }
      : current),
    DesktopNewChat: () => SubscriptionRef.update(state, (current): SpikeState => ({
      ...current,
      workspace: "chat" as const,
      selectedSessionRef: null,
      input: "",
      messages: [],
      pending: false,
      revision: nextRevision(current),
    })),
    DesktopWorkspaceSelected: (workspace: "chat" | "home") => SubscriptionRef.update(state, (current): SpikeState => ({
      ...current,
      workspace,
      revision: nextRevision(current),
    })),
    DesktopSettingsToggled: () => SubscriptionRef.update(state, (current): SpikeState => ({
      ...current,
      workspace: "settings",
      revision: nextRevision(current),
    })),
    DesktopChatSelected: (sessionRef: string) => SubscriptionRef.update(state, (current): SpikeState => {
      if (!fixtureSessions.some((session) => session.ref === sessionRef)) return current;
      return {
        ...current,
        workspace: "chat" as const,
        selectedSessionRef: sessionRef,
        messages: sessionRef === fixtureSessions[0].ref ? initialMessages : [],
        pending: false,
        revision: nextRevision(current),
      };
    }),
  });
  return { state, registry, program: makeViewProgramFromState(state, spikeView) };
});
