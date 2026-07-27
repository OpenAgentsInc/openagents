import { Effect, Schema, SubscriptionRef, type Stream } from "@effect-native/core/effect";
import {
  Badge,
  Button,
  Card,
  ComponentValueBinding,
  defineIntent,
  Divider,
  IntentRef,
  List,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  Stack,
  StaticPayload,
  Text,
  TextField,
  type IntentHandlers,
  type IntentReporter,
  type KeyedView,
  type View,
} from "@effect-native/core";
import type { Issue31CommandArguments } from "@openagentsinc/sarah/issue31-nostr";

import type {
  OmegaBridgeAnnouncement,
  OmegaBridgePairingBootstrap,
  OmegaDeviceBridgeClient,
  OmegaDeviceBridgeState,
  OmegaMirrorRun,
  OmegaMirrorThread,
} from "../workroom/omega-device-bridge-client";

const EmptyPayload = Schema.Struct({});

export const OmegaPairDesktopPressed = defineIntent("OmegaPairDesktopPressed", EmptyPayload);
export const OmegaActivitySelected = defineIntent(
  "OmegaActivitySelected",
  Schema.Struct({ threadRef: Schema.String }),
);
export const OmegaThreadClosed = defineIntent("OmegaThreadClosed", EmptyPayload);
export const OmegaThreadDraftChanged = defineIntent("OmegaThreadDraftChanged", Schema.String);
export const OmegaThreadEnqueuePressed = defineIntent("OmegaThreadEnqueuePressed", EmptyPayload);
export const OmegaThreadSteerPressed = defineIntent("OmegaThreadSteerPressed", EmptyPayload);

export const omegaMobileHomeIntentDefinitions = [
  OmegaPairDesktopPressed,
  OmegaActivitySelected,
  OmegaThreadClosed,
  OmegaThreadDraftChanged,
  OmegaThreadEnqueuePressed,
  OmegaThreadSteerPressed,
] as const;

export type OmegaMobileHomeState = Readonly<{
  bridge: OmegaDeviceBridgeState;
  selectedThreadRef: string | null;
  observedAt: number;
  notice: string | null;
  threadDraft: string;
  commandLaneAvailable: boolean;
  commandNotice: string | null;
}>;

export type OmegaMobileHomeConnectRequest = Readonly<{
  announcements: ReadonlyArray<OmegaBridgeAnnouncement>;
  pairing: OmegaBridgePairingBootstrap | null;
  manualMagicDns: string | null;
}>;

type Activity =
  | Readonly<{ type: "thread"; updatedAt: number; thread: OmegaMirrorThread }>
  | Readonly<{ type: "run"; updatedAt: number; run: OmegaMirrorRun }>;

const connectionLabel = (state: OmegaDeviceBridgeState): string => {
  switch (state.connection.state) {
    case "direct":
      return "Direct";
    case "relay":
      return "Relay";
    case "offline":
      return "Offline";
  }
};

const connectionTone = (state: OmegaDeviceBridgeState): "success" | "info" | "danger" => {
  switch (state.connection.state) {
    case "direct":
      return "success";
    case "relay":
      return "info";
    case "offline":
      return "danger";
  }
};

const relativeStaleness = (state: OmegaMobileHomeState): string => {
  if (state.bridge.connection.state === "direct") return "Live from your desktop";
  const staleSince = state.bridge.connection.staleSince ?? state.bridge.mirror?.projectedAt ?? null;
  if (staleSince === null) {
    return state.bridge.connection.state === "relay"
      ? "Relay is available. The desktop mirror is not current."
      : "The desktop is unreachable.";
  }
  const elapsedSeconds = Math.max(0, Math.floor((state.observedAt - staleSince) / 1_000));
  if (elapsedSeconds < 60) return `Last desktop update ${elapsedSeconds}s ago`;
  const elapsedMinutes = Math.floor(elapsedSeconds / 60);
  if (elapsedMinutes < 60) return `Last desktop update ${elapsedMinutes}m ago`;
  return `Last desktop update ${Math.floor(elapsedMinutes / 60)}h ago`;
};

const executorLabel = (thread: OmegaMirrorThread): string => {
  const model = thread.executor.modelName ?? thread.executor.modelId;
  return model === null
    ? thread.executor.executorName
    : `${thread.executor.executorName} · ${model}`;
};

const activity = (state: OmegaMobileHomeState): ReadonlyArray<Activity> =>
  [
    ...(state.bridge.mirror?.threads.map(
      (thread): Activity => ({ type: "thread", updatedAt: thread.updatedAt, thread }),
    ) ?? []),
    ...(state.bridge.mirror?.runs.map(
      (run): Activity => ({ type: "run", updatedAt: run.updatedAt, run }),
    ) ?? []),
  ].toSorted((left, right) => right.updatedAt - left.updatedAt);

const activityRow = (entry: Activity): KeyedView => {
  if (entry.type === "thread") {
    const thread = entry.thread;
    return Button({
      key: `omega-activity-thread-${thread.threadRef}`,
      label: `${thread.title}\n${executorLabel(thread)} · ${thread.state}`,
      variant: "ghost",
      onPress: IntentRef("OmegaActivitySelected", StaticPayload({ threadRef: thread.threadRef })),
      a11y: {
        label: `${thread.title}. ${executorLabel(thread)}. ${thread.state}. Open read-only thread.`,
      },
      style: {
        width: "full",
        minHeight: "md",
        borderRadius: "md",
        backgroundColor: "surface",
      },
    }) as KeyedView;
  }
  return Card(
    {
      key: `omega-activity-run-${entry.run.runRef}`,
      a11y: {
        label: `${entry.run.title}. Omega, ${entry.run.lane}. ${entry.run.state}.`,
      },
      style: {
        width: "full",
        padding: "3",
        borderRadius: "md",
        backgroundColor: "surface",
      },
    },
    [
      Text({
        key: `omega-activity-run-title-${entry.run.runRef}`,
        content: entry.run.title,
        variant: "label",
        color: "textPrimary",
        weight: "semibold",
      }),
      Text({
        key: `omega-activity-run-detail-${entry.run.runRef}`,
        content: `Omega · ${entry.run.lane} · ${entry.run.state}`,
        variant: "caption",
        color: "textMuted",
      }),
    ],
  ) as KeyedView;
};

const connectionHeader = (state: OmegaMobileHomeState): View =>
  Stack(
    {
      key: "omega-desktop-header",
      direction: "column",
      gap: "1.5",
      padding: "4",
      style: {
        width: "full",
        backgroundColor: "surface",
      },
      a11y: {
        role: "region",
        label: `${state.bridge.mirror?.desktopName ?? "Omega desktop"}. ${connectionLabel(
          state.bridge,
        )}. ${relativeStaleness(state)}`,
      },
    },
    [
      Stack(
        {
          key: "omega-desktop-header-line",
          direction: "row",
          align: "center",
          justify: "between",
          gap: "3",
          style: { width: "full" },
        },
        [
          Text({
            key: "omega-desktop-name",
            content: state.bridge.mirror?.desktopName ?? "Omega desktop",
            variant: "title",
            color: "textPrimary",
            weight: "bold",
            style: { flex: 1 },
          }),
          Badge({
            key: "omega-desktop-connection",
            label: connectionLabel(state.bridge),
            tone: connectionTone(state.bridge),
            variant: "soft",
            size: "sm",
          }),
        ],
      ),
      Text({
        key: "omega-desktop-staleness",
        content: relativeStaleness(state),
        variant: "caption",
        color: "textMuted",
      }),
    ],
  );

const pairingView = (state: OmegaMobileHomeState): View =>
  Stack(
    {
      key: "omega-pairing",
      direction: "column",
      align: "stretch",
      justify: "center",
      gap: "4",
      padding: "6",
      style: { width: "full", height: "full", maxWidth: "lg", alignSelf: "center" },
      a11y: { role: "region", label: "Pair an Omega desktop" },
    },
    [
      Text({
        key: "omega-pairing-title",
        content: "Mirror your desktop",
        variant: "heading",
        color: "textPrimary",
        weight: "bold",
      }),
      Text({
        key: "omega-pairing-copy",
        content:
          "Open Omega on your desktop, show its device QR code, and scan it here. The phone keeps only the connection grant and resume cursor.",
        variant: "body",
        color: "textMuted",
      }),
      Button({
        key: "omega-pairing-action",
        label: "Scan desktop QR",
        variant: "primary",
        onPress: IntentRef("OmegaPairDesktopPressed", StaticPayload({})),
        a11y: { label: "Scan the Omega desktop pairing QR code" },
        style: { width: "full", minHeight: "md" },
      }),
      ...(state.notice === null
        ? []
        : [
            Text({
              key: "omega-pairing-notice",
              content: state.notice,
              variant: "caption",
              color: "textMuted",
            }),
          ]),
    ],
  );

const emptyFeed = (): View =>
  Stack(
    {
      key: "omega-empty-feed",
      direction: "column",
      gap: "2",
      padding: "5",
      style: { width: "full" },
    },
    [
      Text({
        key: "omega-empty-feed-title",
        content: "No desktop activity yet",
        variant: "title",
        color: "textPrimary",
        weight: "semibold",
      }),
      Text({
        key: "omega-empty-feed-copy",
        content: "Threads and runs appear here when Omega reports them.",
        variant: "body",
        color: "textMuted",
      }),
    ],
  );

const feedView = (state: OmegaMobileHomeState): View => {
  const entries = activity(state);
  return Stack(
    {
      key: "omega-feed",
      direction: "column",
      gap: "3",
      padding: "4",
      style: { width: "full", height: "full", maxWidth: "xl", alignSelf: "center" },
      a11y: { role: "region", label: "Live Omega desktop activity" },
    },
    [
      Text({
        key: "omega-feed-title",
        content: "Desktop activity",
        variant: "title",
        color: "textPrimary",
        weight: "semibold",
      }),
      ...(state.notice === null
        ? []
        : [
            Text({
              key: "omega-feed-notice",
              content: state.notice,
              variant: "caption",
              color: "textMuted",
            }),
          ]),
      ...(entries.length === 0
        ? [emptyFeed()]
        : [
            List(
              {
                key: "omega-activity-list",
                virtualize: true,
                estimatedItemSize: "md",
                style: { width: "full", height: "full" },
              },
              entries.map(activityRow),
            ),
          ]),
    ],
  );
};

const threadView = (state: OmegaMobileHomeState, thread: OmegaMirrorThread): View =>
  Stack(
    {
      key: "omega-thread-view",
      direction: "column",
      gap: "3",
      padding: "4",
      style: { width: "full", height: "full", maxWidth: "xl", alignSelf: "center" },
      a11y: {
        role: "region",
        label: `${thread.title}, ${
          state.commandLaneAvailable ? "signed command composer" : "read-only desktop thread"
        }`,
      },
    },
    [
      Button({
        key: "omega-thread-back",
        label: "Back to activity",
        variant: "ghost",
        onPress: IntentRef("OmegaThreadClosed", StaticPayload({})),
        style: { minHeight: "sm", alignSelf: "start" },
      }),
      Text({
        key: "omega-thread-title",
        content: thread.title,
        variant: "heading",
        color: "textPrimary",
        weight: "bold",
      }),
      Text({
        key: "omega-thread-disclosure",
        content: `${executorLabel(thread)} · ${thread.state} · ${
          state.commandLaneAvailable ? "Signed owner commands" : "Read only"
        }`,
        variant: "caption",
        color: "textMuted",
      }),
      Divider({ key: "omega-thread-divider", orientation: "horizontal" }),
      ...(thread.transcript.length === 0
        ? [
            Text({
              key: "omega-thread-empty",
              content: "No transcript entries are available.",
              variant: "body",
              color: "textMuted",
            }),
          ]
        : [
            List(
              {
                key: "omega-thread-transcript",
                virtualize: true,
                estimatedItemSize: "md",
                style: { width: "full", height: "full" },
              },
              thread.transcript.map(
                (message) =>
                  Card(
                    {
                      key: `omega-thread-message-${message.messageRef}`,
                      style: {
                        width: "full",
                        padding: "3",
                        borderRadius: "md",
                        backgroundColor: message.role === "user" ? "surfaceRaised" : "surface",
                      },
                    },
                    [
                      Text({
                        key: `omega-thread-message-role-${message.messageRef}`,
                        content: message.role,
                        variant: "caption",
                        color: "textMuted",
                        weight: "semibold",
                      }),
                      Text({
                        key: `omega-thread-message-body-${message.messageRef}`,
                        content: message.text,
                        variant: "body",
                        color: "textPrimary",
                      }),
                    ],
                  ) as KeyedView,
              ),
            ),
          ]),
      ...(state.commandLaneAvailable
        ? [
            Divider({ key: "omega-thread-composer-divider", orientation: "horizontal" }),
            TextField({
              key: "omega-thread-composer",
              value: state.threadDraft,
              label: "Message Omega",
              placeholder: "Send work to this desktop thread",
              multiline: true,
              onChange: IntentRef("OmegaThreadDraftChanged", ComponentValueBinding()),
              variant: "outline",
              size: "md",
              style: { width: "full" },
            }),
            Stack(
              {
                key: "omega-thread-command-actions",
                direction: "row",
                gap: "2",
                style: { width: "full" },
              },
              [
                Button({
                  key: "omega-thread-enqueue",
                  label: "Send / enqueue",
                  variant: "primary",
                  onPress: IntentRef("OmegaThreadEnqueuePressed", StaticPayload({})),
                  style: { flex: 1, minHeight: "md" },
                  a11y: {
                    label: "Send now if idle, or enqueue after the current desktop turn completes",
                  },
                }),
                Button({
                  key: "omega-thread-steer",
                  label: "Steer at boundary",
                  variant: "secondary",
                  onPress: IntentRef("OmegaThreadSteerPressed", StaticPayload({})),
                  style: { flex: 1, minHeight: "md" },
                  a11y: {
                    label: "Request delivery at the current executor's declared steer boundary",
                  },
                }),
              ],
            ),
            ...(state.commandNotice === null
              ? []
              : [
                  Text({
                    key: "omega-thread-command-notice",
                    content: state.commandNotice,
                    variant: "caption",
                    color: "textMuted",
                  }),
                ]),
          ]
        : []),
    ],
  );

export const renderOmegaMobileHome = (state: OmegaMobileHomeState): View => {
  const selectedThread =
    state.bridge.mirror?.threads.find((thread) => thread.threadRef === state.selectedThreadRef) ??
    null;
  return Stack(
    {
      key: "omega-mobile-home",
      direction: "column",
      gap: "0",
      style: { width: "full", height: "full", backgroundColor: "background" },
      a11y: { role: "region", label: "Omega mobile desktop mirror" },
    },
    [
      connectionHeader(state),
      !state.bridge.paired
        ? pairingView(state)
        : selectedThread === null
          ? feedView(state)
          : threadView(state, selectedThread),
    ],
  );
};

export type OmegaMobileHomeProgram = Readonly<{
  initialState: OmegaMobileHomeState;
  viewStream: Stream.Stream<View>;
  report: IntentReporter;
  close: () => Promise<void>;
}>;

type PublishCommandIntent = (
  request: Readonly<{
    idempotencyRef: string;
    arguments: Issue31CommandArguments;
  }>,
) => Promise<unknown>;

const publishThreadCommand = (
  disposition: "enqueue" | "steer",
  state: SubscriptionRef.SubscriptionRef<OmegaMobileHomeState>,
  publishCommandIntent: PublishCommandIntent | undefined,
) =>
  Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state);
    const text = current.threadDraft.trim();
    const threadRef = current.selectedThreadRef;
    if (publishCommandIntent === undefined || threadRef === null) {
      yield* SubscriptionRef.update(state, (value) => ({
        ...value,
        commandNotice: "The signed Omega command lane is unavailable.",
      }));
      return;
    }
    if (text.length === 0) {
      yield* SubscriptionRef.update(state, (value) => ({
        ...value,
        commandNotice: "Write a message before sending.",
      }));
      return;
    }
    yield* SubscriptionRef.update(state, (value) => ({
      ...value,
      commandNotice: `Publishing a signed ${disposition} command…`,
    }));
    const idempotencyRef = `idempotency.issue31.agent_thread_message:${globalThis.crypto.randomUUID().replaceAll("-", "")}`;
    const published = yield* Effect.tryPromise({
      try: () =>
        publishCommandIntent({
          idempotencyRef,
          arguments: {
            kind: "agent_thread_message",
            actionRef: "action.issue31.omega.agent_thread_message",
            threadRef,
            text,
            disposition,
          },
        }),
      catch: (error) =>
        error instanceof Error ? error : new Error("The signed command could not be published."),
    }).pipe(
      Effect.as(true),
      Effect.catch(() => Effect.succeed(false)),
    );
    if (!published) {
      yield* SubscriptionRef.update(state, (value) => ({
        ...value,
        commandNotice: "The signed command could not be published.",
      }));
      return;
    }
    yield* SubscriptionRef.update(state, (value) => ({
      ...value,
      threadDraft: "",
      commandNotice:
        disposition === "steer"
          ? "Signed steer published. Omega will report the executor's declared boundary outcome."
          : "Signed send published. Omega will send now or enqueue behind the running turn.",
    }));
  });

export const buildOmegaMobileHomeProgram = (
  input: Readonly<{
    bridge: OmegaDeviceBridgeClient;
    connectRequest: OmegaMobileHomeConnectRequest;
    scanPairing: () => Promise<OmegaBridgePairingBootstrap | null>;
    publishCommandIntent?: PublishCommandIntent;
    now?: () => number;
  }>,
): OmegaMobileHomeProgram =>
  Effect.runSync(
    Effect.gen(function* () {
      const now = input.now ?? Date.now;
      const initialState: OmegaMobileHomeState = {
        bridge: input.bridge.state(),
        selectedThreadRef: null,
        observedAt: now(),
        notice: null,
        threadDraft: "",
        commandLaneAvailable: input.publishCommandIntent !== undefined,
        commandNotice: null,
      };
      const state = yield* SubscriptionRef.make(initialState);
      const handlers: IntentHandlers<typeof omegaMobileHomeIntentDefinitions> = {
        OmegaPairDesktopPressed: () =>
          Effect.gen(function* () {
            const pairing = yield* Effect.tryPromise({
              try: input.scanPairing,
              catch: (error) =>
                error instanceof Error ? error : new Error("The QR scanner is unavailable."),
            }).pipe(
              Effect.catch((error) =>
                SubscriptionRef.update(state, (current) => ({
                  ...current,
                  notice: error.message,
                  observedAt: now(),
                })).pipe(Effect.as(null)),
              ),
            );
            if (pairing === null) return;
            yield* input.bridge.connect({ ...input.connectRequest, pairing }).pipe(
              Effect.catch((error) =>
                SubscriptionRef.update(state, (current) => ({
                  ...current,
                  notice: error.message,
                  observedAt: now(),
                })),
              ),
            );
          }),
        OmegaActivitySelected: ({ threadRef }) =>
          SubscriptionRef.update(state, (current) => ({
            ...current,
            selectedThreadRef: current.bridge.mirror?.threads.some(
              (thread) => thread.threadRef === threadRef,
            )
              ? threadRef
              : null,
            threadDraft: "",
            commandNotice: null,
          })),
        OmegaThreadClosed: () =>
          SubscriptionRef.update(state, (current) => ({
            ...current,
            selectedThreadRef: null,
            threadDraft: "",
            commandNotice: null,
          })),
        OmegaThreadDraftChanged: (value: string) =>
          SubscriptionRef.update(state, (current) => ({
            ...current,
            threadDraft: value.slice(0, 12_000),
          })),
        OmegaThreadEnqueuePressed: () =>
          publishThreadCommand("enqueue", state, input.publishCommandIntent),
        OmegaThreadSteerPressed: () =>
          publishThreadCommand("steer", state, input.publishCommandIntent),
      };
      const registry = yield* makeIntentRegistry(omegaMobileHomeIntentDefinitions, handlers);
      const report: IntentReporter = (ref, runtimeValue) =>
        registry.dispatch(resolveIntentRef(ref, runtimeValue));
      const unsubscribe = input.bridge.subscribe((bridge) => {
        Effect.runFork(
          SubscriptionRef.update(state, (current) => ({
            ...current,
            bridge,
            observedAt: now(),
            selectedThreadRef:
              current.selectedThreadRef !== null &&
              bridge.mirror?.threads.some(
                (thread) => thread.threadRef === current.selectedThreadRef,
              )
                ? current.selectedThreadRef
                : null,
          })),
        );
      });
      Effect.runFork(
        input.bridge.connect(input.connectRequest).pipe(
          Effect.catch((error) =>
            SubscriptionRef.update(state, (current) => ({
              ...current,
              notice:
                error.reason === "all_endpoints_failed" && !current.bridge.paired
                  ? null
                  : error.message,
              observedAt: now(),
            })),
          ),
        ),
      );
      return {
        initialState,
        viewStream: makeViewProgramFromState(state, renderOmegaMobileHome).viewStream,
        report,
        close: async () => {
          unsubscribe();
          await Effect.runPromise(input.bridge.close());
        },
      };
    }),
  );
