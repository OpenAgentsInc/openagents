import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { useQuery } from "convex/react";
import * as Crypto from "expo-crypto";
import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  ActivityIndicator,
  FlatList,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  TextInput,
  View,
  useWindowDimensions,
  type ListRenderItemInfo,
  type NativeScrollEvent,
  type NativeSyntheticEvent,
  type TextStyle,
  type ViewStyle,
} from "react-native";

import { SarahVoiceScreen } from "../screens/sarah-voice-screen";
import { useMobileClientOutbox } from "../outbox/client-outbox-provider";
import { Button } from "../ui/button";
import { Text } from "../ui/text";
import { colors, radius, spacing, typography } from "../ui/theme";
import {
  decodeAttentionInbox,
  decodeWorkComposerDraft,
  decodeWorkTranscriptPage,
  decodeWorkShell,
  type AttentionShell,
  type ControllerTarget,
  type WorkComposerContext,
  type WorkComposerDraft,
  type WorkTranscriptRow,
} from "./contracts";
import { attentionInboxQuery, workShellQuery, workTranscriptQuery } from "./convex-functions";
import { initialFeedAnchorState, reduceFeedAnchor, shouldMaintainFeedEnd } from "./feed-anchor";
import { controllerLayout } from "./layout";
import type { ControllerRouteParams } from "./routes";
import { useControllerSession } from "./session-provider";
import { createSubmissionGuard } from "./submission-guard";

type HomeProps = NativeStackScreenProps<ControllerRouteParams, "Home">;
type ThreadProps = NativeStackScreenProps<ControllerRouteParams, "Thread">;

const formatAge = (timestamp: number): string => {
  const seconds = Math.max(0, Math.floor((Date.now() - timestamp) / 1_000));
  if (seconds < 60) return "now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `${hours}h` : `${Math.floor(hours / 24)}d`;
};

const mobileContextSourceRef = (kind: WorkComposerContext["kind"], label: string): string => {
  const safe = label
    .trim()
    .replace(/[^A-Za-z0-9._:/-]+/gu, "-")
    .replace(/^-+|-+$/gu, "")
    .slice(0, 140);
  return `${kind}:${safe || "context"}`;
};

const attentionTone = (state: string): string => {
  switch (state) {
    case "approval":
      return colors.warn;
    case "failed":
      return colors.fault;
    case "working":
      return colors.accentInk;
    case "done_unseen":
      return colors.live;
    default:
      return colors.textDim;
  }
};

const WorkspaceHeader = ({ onSettings }: { readonly onSettings: () => void }) => {
  const session = useControllerSession();
  if (session.phase !== "ready") return null;
  return (
    <View style={$workspaceHeader}>
      <View style={$workspaceIdentity}>
        <View style={$workspaceMark} accessibilityElementsHidden>
          <Text preset="label" color={colors.accentInk}>
            OA
          </Text>
        </View>
        <View style={$grow}>
          <Text preset="label">WORKSPACE</Text>
          <Text preset="subheading" numberOfLines={1}>
            {session.bootstrap.workspace.label}
          </Text>
        </View>
      </View>
      <Button label="Settings" preset="ghost" onPress={onSettings} />
    </View>
  );
};

const InboxRow = ({
  shell,
  onPress,
}: {
  readonly shell: AttentionShell;
  readonly onPress: () => void;
}) => (
  <Pressable
    accessibilityRole="button"
    accessibilityLabel={`${shell.label}. ${shell.attentionState}. ${shell.status}. Open work.`}
    onPress={onPress}
    style={({ pressed }) => [$inboxRow, pressed ? $rowPressed : null]}
  >
    <View style={[$attentionRail, { backgroundColor: attentionTone(shell.attentionState) }]} />
    <View style={$grow}>
      <View style={$rowBetween}>
        <Text preset="bodyStrong" numberOfLines={1} style={$grow}>
          {shell.label}
        </Text>
        <Text preset="label" color={colors.textFaint}>
          {formatAge(shell.updatedAt)}
        </Text>
      </View>
      <View style={$rowMeta}>
        <Text preset="caption" color={attentionTone(shell.attentionState)}>
          {shell.attentionState.replaceAll("_", " ")}
        </Text>
        <Text preset="caption" color={colors.textFaint}>
          {shell.status}
        </Text>
        {shell.pendingRequests.length === 0 ? null : (
          <Text preset="caption" color={colors.warn}>
            {`${shell.pendingRequests.length} waiting`}
          </Text>
        )}
      </View>
    </View>
  </Pressable>
);

const ControllerSidebar = ({
  rows,
  loading,
  selectedId,
  onSelect,
  onNewTask,
  onConnections,
}: {
  readonly rows: ReadonlyArray<AttentionShell>;
  readonly loading: boolean;
  readonly selectedId: string | null;
  readonly onSelect: (shell: AttentionShell) => void;
  readonly onNewTask: () => void;
  readonly onConnections: () => void;
}) => {
  const renderItem = useCallback(
    ({ item }: ListRenderItemInfo<AttentionShell>) => (
      <View style={selectedId === item.aggregateId ? $selectedRow : null}>
        <InboxRow shell={item} onPress={() => onSelect(item)} />
      </View>
    ),
    [onSelect, selectedId],
  );
  return (
    <View style={$sidebar}>
      <View style={$sidebarTitle}>
        <View>
          <Text preset="heading">Attention</Text>
          <Text preset="caption">Live from Convex</Text>
        </View>
        <View style={$headerActions}>
          <Button label="Connections" preset="ghost" onPress={onConnections} />
          <Button label="New" preset="secondary" onPress={onNewTask} />
        </View>
      </View>
      {loading ? (
        <View style={$center}>
          <ActivityIndicator color={colors.accent} />
          <Text preset="caption">Synchronizing your workspace…</Text>
        </View>
      ) : rows.length === 0 ? (
        <View style={$empty}>
          <Text preset="subheading">Nothing needs you.</Text>
          <Text preset="body" color={colors.textDim}>
            Active work and requests will appear here without a refresh.
          </Text>
        </View>
      ) : (
        <FlatList
          data={rows}
          renderItem={renderItem}
          keyExtractor={(item) => `${item.aggregateType}:${item.aggregateId}`}
          contentContainerStyle={$inboxContent}
          initialNumToRender={12}
          maxToRenderPerBatch={12}
          windowSize={7}
        />
      )}
    </View>
  );
};

export const ControllerHomeScreen = ({ navigation }: HomeProps) => {
  const raw = useQuery(attentionInboxQuery, { limit: 100 });
  const rows = useMemo(() => (raw === undefined ? [] : decodeAttentionInbox(raw)), [raw]);
  const { width, height } = useWindowDimensions();
  const layout = controllerLayout(width, height);
  const [selected, setSelected] = useState<AttentionShell | null>(null);

  const select = useCallback(
    (shell: AttentionShell) => {
      if (layout.mode === "split") {
        setSelected(shell);
        return;
      }
      navigation.navigate("Thread", {
        aggregateType: shell.aggregateType,
        aggregateId: shell.aggregateId,
        label: shell.label,
      });
    },
    [layout.mode, navigation],
  );

  return (
    <View style={$root}>
      <WorkspaceHeader onSettings={() => navigation.navigate("Settings")} />
      <View style={$adaptiveRow}>
        <View style={layout.mode === "split" ? { width: layout.sidebarWidth } : $grow}>
          <ControllerSidebar
            rows={rows}
            loading={raw === undefined}
            selectedId={selected?.aggregateId ?? null}
            onSelect={select}
            onNewTask={() => navigation.navigate("NewTask")}
            onConnections={() => navigation.navigate("Connections")}
          />
        </View>
        {layout.mode === "split" ? (
          <View style={[$splitDetail, { maxWidth: layout.chatWidth }]}>
            {selected === null ? (
              <View style={$emptySelection}>
                <Text preset="display">Choose what needs you.</Text>
                <Text preset="body" color={colors.textDim}>
                  Open a thread to read its live history or steer the current turn.
                </Text>
              </View>
            ) : (
              <ThreadPane
                target={selected}
                onOpenSurface={(surface) =>
                  navigation.navigate(surface, { aggregateId: selected.aggregateId })
                }
              />
            )}
          </View>
        ) : null}
      </View>
    </View>
  );
};

const semanticRowText = (row: WorkTranscriptRow): string => {
  switch (row.kind) {
    case "user":
    case "assistant":
      return row.text;
    case "work_entry":
      return row.summary ?? row.body ?? row.title;
    case "turn_boundary":
      return row.label;
    case "folded_turn":
      return `${row.summary} · ${row.rowCount} rows`;
    case "proposed_plan":
      return row.title;
    case "working_indicator":
      return row.label;
    case "provider_error":
    case "runtime_error":
      return row.summary;
    case "approval_request":
    case "input_request":
      return row.summary;
  }
};

const DetailRow = ({
  row,
  onPlanAction,
}: {
  readonly row: WorkTranscriptRow;
  readonly onPlanAction: (
    row: Extract<WorkTranscriptRow, { readonly kind: "proposed_plan" }>,
    action: "refine" | "implement",
  ) => void;
}) => {
  const user = row.kind === "user";
  if (row.kind === "approval_request" || row.kind === "input_request") return null;
  return (
    <View style={user ? $userTurn : $agentTurn}>
      <Text preset="label" color={user ? colors.accentInk : colors.textFaint}>
        {user ? "YOU" : row.kind.replaceAll("_", " ").toUpperCase()}
      </Text>
      <Text preset={user ? "mono" : "body"}>{semanticRowText(row)}</Text>
      {row.kind === "user" && row.context.length > 0 ? (
        <View style={$rowMeta}>
          {row.context.map((context) => (
            <Text
              key={`${context.kind}:${context.sourceRef}`}
              preset="caption"
              color={colors.accentInk}
            >
              {`${context.kind}: ${context.label}`}
            </Text>
          ))}
        </View>
      ) : null}
      {row.kind === "proposed_plan" ? (
        <View>
          {row.steps.map((step) => (
            <Text
              key={step.id}
              preset="caption"
              color={step.state === "completed" ? colors.live : colors.textDim}
            >
              {`${step.state === "completed" ? "✓" : "○"} ${step.label}`}
            </Text>
          ))}
          {row.completed ? (
            <View style={$requestActions}>
              <Button
                label="Refine"
                preset="secondary"
                onPress={() => onPlanAction(row, "refine")}
              />
              <Button label="Implement" onPress={() => onPlanAction(row, "implement")} />
            </View>
          ) : null}
        </View>
      ) : null}
      <Text preset="label" color={colors.textFaint}>
        {`${row.state} · ${formatAge(row.updatedAtMs)}`}
      </Text>
    </View>
  );
};

const PendingRequestCard = ({
  row,
  busy,
  onRespond,
  onDisclosure,
}: {
  readonly row: Extract<WorkTranscriptRow, { readonly kind: "approval_request" | "input_request" }>;
  readonly busy: boolean;
  readonly onRespond: (response: string | boolean) => void;
  readonly onDisclosure: (open: boolean) => void;
}) => {
  const approval = row.kind === "approval_request";
  const [answer, setAnswer] = useState("");
  return (
    <View style={$requestCard} accessibilityRole="alert">
      <Text preset="label" color={colors.warn}>
        {approval ? "APPROVAL REQUIRED" : "INPUT REQUIRED"}
      </Text>
      <Text preset="bodyStrong">{row.summary}</Text>
      <Text preset="caption" color={colors.textDim}>{`${row.commandName} · ${row.effect}`}</Text>
      {approval ? (
        <View style={$requestActions}>
          <Button label="Approve" disabled={busy} onPress={() => onRespond(true)} />
          <Button
            label="Decline"
            preset="secondary"
            disabled={busy}
            onPress={() => onRespond(false)}
          />
        </View>
      ) : (
        <View style={$inputAnswer}>
          <TextInput
            accessibilityLabel="Answer the input request"
            value={answer}
            onChangeText={setAnswer}
            onFocus={() => onDisclosure(true)}
            onBlur={() => onDisclosure(false)}
            placeholder="Type your answer"
            placeholderTextColor={colors.textFaint}
            multiline
            style={$input}
          />
          <Button
            label="Answer"
            disabled={busy || answer.trim() === ""}
            onPress={() => onRespond(answer.trim())}
          />
        </View>
      )}
    </View>
  );
};

const ThreadComposer = ({
  target,
  running,
  pendingRequest,
  preset,
  shellLive,
  onDisclosure,
}: {
  readonly target: ControllerTarget;
  readonly running: boolean;
  readonly pendingRequest: Extract<
    WorkTranscriptRow,
    { readonly kind: "approval_request" | "input_request" }
  > | null;
  readonly preset: Readonly<{
    key: string;
    text: string;
    context: ReadonlyArray<WorkComposerContext>;
  }> | null;
  readonly shellLive: boolean;
  readonly onDisclosure: (open: boolean) => void;
}) => {
  const session = useControllerSession();
  const outbox = useMobileClientOutbox();
  const [draft, setDraft] = useState("");
  const [context, setContext] = useState<Array<WorkComposerContext>>([]);
  const [contextKind, setContextKind] = useState<WorkComposerContext["kind"]>("file");
  const [contextLabel, setContextLabel] = useState("");
  const [expanded, setExpanded] = useState(false);
  const [phase, setPhase] = useState<"idle" | "submitting">("idle");
  const [notice, setNotice] = useState<string | null>(null);
  const submissionGuard = useRef(createSubmissionGuard());
  const loadedDraftKey = useRef<string | null>(null);
  const appliedPreset = useRef<string | null>(null);
  const observationKey = `composer:${target.aggregateType}:${target.aggregateId}`;

  useEffect(() => {
    if (outbox.phase !== "ready" || loadedDraftKey.current === observationKey) return;
    loadedDraftKey.current = observationKey;
    void outbox.runtime
      .observation({ key: observationKey, connected: true, synchronizing: false })
      .then((observation) => {
        if (observation === null) return;
        const restored = decodeWorkComposerDraft(observation.value);
        setDraft(restored.text);
        setContext([...restored.context]);
      })
      .catch(() => setNotice("The saved draft could not be restored."));
  }, [observationKey, outbox]);

  useEffect(() => {
    if (preset === null || appliedPreset.current === preset.key) return;
    appliedPreset.current = preset.key;
    setDraft(preset.text);
    setContext([...preset.context]);
    setExpanded(true);
  }, [preset]);

  useEffect(() => {
    if (outbox.phase !== "ready" || loadedDraftKey.current !== observationKey) return;
    const timeout = setTimeout(() => {
      const value: WorkComposerDraft = {
        schemaVersion: "openagents.composer_draft.v1",
        aggregateType: target.aggregateType,
        aggregateId: target.aggregateId,
        text: draft.slice(0, 8_000),
        context: context.slice(0, 16),
        updatedAtMs: Date.now(),
      };
      if (value.text.trim() === "" && value.context.length === 0) {
        void outbox.runtime.removeObservation(observationKey);
      } else {
        void outbox.runtime.cacheObservation(observationKey, value);
      }
    }, 250);
    return () => clearTimeout(timeout);
  }, [context, draft, observationKey, outbox, target]);

  const guarded = useCallback(async (task: () => Promise<void>) => {
    if (submissionGuard.current.phase() !== "idle") return;
    setNotice(null);
    try {
      await submissionGuard.current.run(task, setPhase);
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "The command could not be sent.");
    }
  }, []);

  const respond = useCallback(
    (response: string | boolean) => {
      if (pendingRequest === null) return;
      void guarded(async () => {
        const operation =
          pendingRequest.kind === "approval_request" ? "approval.respond" : "input.respond";
        const summary = await session.enqueueAndDrain(
          {
            commandId: `cmd_mobile_${Crypto.randomUUID()}`,
            operation,
            orderingKey: `${target.aggregateType}:${target.aggregateId}`,
            payload: {
              ...target,
              requestId: pendingRequest.requestId,
              decisionRevision: pendingRequest.decisionRevision,
              expiresAtMs: pendingRequest.expiresAtMs,
              response,
            },
            createdAtMs: Date.now(),
            decisionRevision: pendingRequest.decisionRevision,
            expiresAtMs: pendingRequest.expiresAtMs,
          },
          {
            shellLive,
            decisionRevisions: { [operation]: pendingRequest.decisionRevision },
          },
        );
        setNotice(summary.delivered > 0 ? "Response received." : "Response queued for delivery.");
      });
    },
    [guarded, pendingRequest, session, shellLive, target],
  );

  const send = useCallback(() => {
    const text = draft.trim();
    if (text === "") return;
    void guarded(async () => {
      const summary = await session.enqueueAndDrain(
        {
          commandId: `cmd_mobile_${Crypto.randomUUID()}`,
          operation: "thread.message.send",
          orderingKey: `${target.aggregateType}:${target.aggregateId}`,
          payload: { ...target, text, context },
          createdAtMs: Date.now(),
        },
        { shellLive, decisionRevisions: {} },
        { clearObservationKey: observationKey },
      );
      setDraft("");
      setContext([]);
      setExpanded(false);
      setNotice(
        summary.delivered > 0 ? "Message queued." : "Saved offline. It will send when live.",
      );
    });
  }, [context, draft, guarded, observationKey, session, shellLive, target]);

  const stop = useCallback(() => {
    void guarded(async () => {
      const receipt = await session.interrupt(`cmd_mobile_${Crypto.randomUUID()}`, target);
      setNotice(receipt.status === "rejected" ? receipt.detail : "Interrupt requested.");
    });
  }, [guarded, session, target]);

  const addContext = useCallback(() => {
    const label = contextLabel.trim();
    if (label === "" || context.length >= 16) return;
    setContext((current) => [
      ...current,
      {
        kind: contextKind,
        sourceRef: mobileContextSourceRef(contextKind, label),
        label: label.slice(0, 160),
      },
    ]);
    setContextLabel("");
  }, [context.length, contextKind, contextLabel]);

  if (pendingRequest !== null) {
    return (
      <View style={$composerRegion}>
        <PendingRequestCard
          row={pendingRequest}
          busy={phase === "submitting"}
          onRespond={respond}
          onDisclosure={onDisclosure}
        />
        {notice === null ? null : <Text preset="caption">{notice}</Text>}
      </View>
    );
  }

  return (
    <View style={$composerRegion}>
      <View style={[$composer, expanded ? $composerExpanded : null]}>
        <TextInput
          accessibilityLabel="Message this work thread"
          value={draft}
          onChangeText={setDraft}
          onFocus={() => setExpanded(true)}
          placeholder="Tell the agent what to do next"
          placeholderTextColor={colors.textFaint}
          multiline={expanded}
          style={$composerInput}
        />
        <Button
          label="Send"
          disabled={phase === "submitting" || draft.trim() === ""}
          onPress={send}
        />
        {running ? (
          <Button label="Stop" preset="danger" disabled={phase === "submitting"} onPress={stop} />
        ) : null}
      </View>
      {context.length === 0 ? null : (
        <View style={$rowMeta}>
          {context.map((item, index) => (
            <Pressable
              accessibilityLabel={`Remove ${item.label} context`}
              accessibilityRole="button"
              key={`${item.kind}:${item.sourceRef}:${index}`}
              onPress={() =>
                setContext((current) => current.filter((_, itemIndex) => itemIndex !== index))
              }
            >
              <Text
                preset="caption"
                color={colors.accentInk}
              >{`${item.kind}: ${item.label} ×`}</Text>
            </Pressable>
          ))}
        </View>
      )}
      {expanded ? (
        <View style={$inputAnswer}>
          <Button
            label={contextKind}
            preset="secondary"
            onPress={() => {
              const kinds: ReadonlyArray<WorkComposerContext["kind"]> = [
                "file",
                "terminal",
                "review",
                "skill",
              ];
              const index = kinds.indexOf(contextKind);
              setContextKind(kinds[(index + 1) % kinds.length] ?? "file");
            }}
          />
          <TextInput
            accessibilityLabel="Context reference"
            value={contextLabel}
            onChangeText={setContextLabel}
            placeholder="Add context"
            placeholderTextColor={colors.textFaint}
            style={$input}
          />
          <Button
            label="Add"
            preset="secondary"
            disabled={contextLabel.trim() === ""}
            onPress={addContext}
          />
        </View>
      ) : null}
      {notice === null ? null : <Text preset="caption">{notice}</Text>}
    </View>
  );
};

const ThreadPane = ({
  target,
  onOpenSurface,
}: {
  readonly target: Pick<AttentionShell, "aggregateType" | "aggregateId" | "label">;
  readonly onOpenSurface: (surface: "Terminal" | "Review" | "Files" | "Git") => void;
}) => {
  const shellRaw = useQuery(workShellQuery, {
    aggregateType: target.aggregateType,
    aggregateId: target.aggregateId,
  });
  const transcriptRaw = useQuery(workTranscriptQuery, {
    aggregateType: target.aggregateType,
    aggregateId: target.aggregateId,
    limit: 200,
  });
  const shell = useMemo(
    () => (shellRaw === undefined ? undefined : decodeWorkShell(shellRaw)),
    [shellRaw],
  );
  const transcript = useMemo(
    () => (transcriptRaw === undefined ? undefined : decodeWorkTranscriptPage(transcriptRaw)),
    [transcriptRaw],
  );
  const pendingRequest = useMemo(() => {
    const rows = transcript?.rows ?? [];
    for (let index = rows.length - 1; index >= 0; index -= 1) {
      const row = rows[index];
      if (
        row !== undefined &&
        row.state === "pending" &&
        (row.kind === "approval_request" || row.kind === "input_request")
      ) {
        return row;
      }
    }
    return null;
  }, [transcript]);
  const listRef = useRef<FlatList<WorkTranscriptRow>>(null);
  const [composerPreset, setComposerPreset] = useState<{
    key: string;
    text: string;
    context: ReadonlyArray<WorkComposerContext>;
  } | null>(null);
  const [anchor, dispatchAnchor] = useReducer(reduceFeedAnchor, initialFeedAnchorState);

  const onScroll = useCallback((event: NativeSyntheticEvent<NativeScrollEvent>) => {
    const { contentOffset, contentSize, layoutMeasurement } = event.nativeEvent;
    dispatchAnchor({
      type: "distance_from_end",
      distance: contentSize.height - (contentOffset.y + layoutMeasurement.height),
    });
  }, []);

  const scrollToEndIfMaintained = useCallback(() => {
    if (!shouldMaintainFeedEnd(anchor)) return;
    listRef.current?.scrollToEnd({ animated: anchor.initialized });
    if (!anchor.initialized) dispatchAnchor({ type: "initial_scroll" });
  }, [anchor]);

  const planAction = useCallback(
    (
      row: Extract<WorkTranscriptRow, { readonly kind: "proposed_plan" }>,
      action: "refine" | "implement",
    ) => {
      setComposerPreset({
        key: `${row.planId}:${row.planRevision}:${action}:${Date.now()}`,
        text:
          action === "refine"
            ? `Refine plan ${row.planId} revision ${row.planRevision}: `
            : `Implement plan ${row.planId} revision ${row.planRevision}.`,
        context: [
          {
            kind: "skill",
            sourceRef: `skill:plan:${row.planId}`.slice(0, 160),
            label: `${row.title} · revision ${row.planRevision}`.slice(0, 160),
          },
        ],
      });
    },
    [],
  );
  const renderDetail = useCallback(
    ({ item }: ListRenderItemInfo<WorkTranscriptRow>) => (
      <DetailRow row={item} onPlanAction={planAction} />
    ),
    [planAction],
  );
  const outline = useMemo(
    () =>
      (transcript?.rows ?? [])
        .map((row, index) => ({ row, index }))
        .filter(({ row }) =>
          [
            "user",
            "proposed_plan",
            "approval_request",
            "input_request",
            "provider_error",
            "runtime_error",
            "turn_boundary",
          ].includes(row.kind),
        )
        .slice(-12),
    [transcript],
  );
  const running = shell?.status === "working" || shell?.status === "queued";
  const targetWithGeneration: ControllerTarget = {
    aggregateType: target.aggregateType,
    aggregateId: target.aggregateId,
    ...(transcript === undefined ? {} : { expectedGeneration: transcript.generation }),
  };

  return (
    <KeyboardAvoidingView
      style={$thread}
      behavior={Platform.OS === "ios" ? "padding" : undefined}
      keyboardVerticalOffset={0}
    >
      <View style={$threadHeader}>
        <View style={$grow}>
          <Text preset="heading" numberOfLines={1}>
            {target.label}
          </Text>
          <Text preset="caption" color={running ? colors.accentInk : colors.textDim}>
            {shell === undefined
              ? "Synchronizing…"
              : `${shell?.status ?? "unavailable"} · generation ${shell?.generation ?? 0}`}
          </Text>
        </View>
        <View style={$surfaceActions}>
          <Button label="Files" preset="ghost" onPress={() => onOpenSurface("Files")} />
          <Button label="Review" preset="ghost" onPress={() => onOpenSurface("Review")} />
          <Button label="Git" preset="ghost" onPress={() => onOpenSurface("Git")} />
          <Button label="Terminal" preset="ghost" onPress={() => onOpenSurface("Terminal")} />
        </View>
      </View>
      {outline.length < 4 ? null : (
        <View style={$outline} accessibilityRole="toolbar">
          <Text preset="label" color={colors.textFaint}>
            JUMP
          </Text>
          {outline.map(({ row, index }) => (
            <Pressable
              accessibilityLabel={`Jump to ${row.kind.replaceAll("_", " ")}`}
              accessibilityRole="button"
              key={`${row.rowId}:${index}`}
              onPress={() => listRef.current?.scrollToIndex({ index, viewPosition: 0.5 })}
              style={$outlineMarker}
            >
              <Text
                preset="label"
                color={
                  row.kind === "approval_request" || row.kind === "input_request"
                    ? colors.warn
                    : row.kind === "provider_error" || row.kind === "runtime_error"
                      ? colors.fault
                      : colors.textDim
                }
              >
                •
              </Text>
            </Pressable>
          ))}
        </View>
      )}
      {transcript === undefined ? (
        <View style={$center}>
          <ActivityIndicator color={colors.accent} />
        </View>
      ) : (
        <FlatList
          ref={listRef}
          data={transcript.rows}
          renderItem={renderDetail}
          keyExtractor={(item) => item.rowId}
          ListHeaderComponent={
            transcript.hasOlder ? (
              <Text preset="caption">Older rows are outside this bounded 200-row window.</Text>
            ) : null
          }
          contentContainerStyle={$feedContent}
          maintainVisibleContentPosition={{ minIndexForVisible: 0, autoscrollToTopThreshold: 24 }}
          onScroll={onScroll}
          scrollEventThrottle={80}
          onContentSizeChange={scrollToEndIfMaintained}
          initialNumToRender={18}
          maxToRenderPerBatch={12}
          windowSize={9}
          removeClippedSubviews={Platform.OS === "android"}
        />
      )}
      <ThreadComposer
        target={targetWithGeneration}
        running={running}
        pendingRequest={pendingRequest}
        preset={composerPreset}
        shellLive={shell !== undefined && shell !== null}
        onDisclosure={(open) => dispatchAnchor({ type: "disclosure", open })}
      />
    </KeyboardAvoidingView>
  );
};

export const ControllerThreadScreen = ({ route, navigation }: ThreadProps) => (
  <View style={$root}>
    <ThreadPane
      target={route.params}
      onOpenSurface={(surface) =>
        navigation.navigate(surface, { aggregateId: route.params.aggregateId })
      }
    />
  </View>
);

export const ControllerSettingsScreen = () => {
  const session = useControllerSession();
  return (
    <View style={$sheet}>
      <Text preset="display">Settings</Text>
      <Text preset="body" color={colors.textDim}>
        {session.phase === "ready"
          ? `Signed in as ${session.bootstrap.actor.name}`
          : "Controller session unavailable"}
      </Text>
      <Button label="Sign out" preset="danger" onPress={() => void session.signOut()} />
    </View>
  );
};

export const ControllerConnectionsScreen = () => {
  const session = useControllerSession();
  return (
    <View style={$sheet}>
      <Text preset="display">Connections</Text>
      <View style={$connectionCard}>
        <View style={$liveDot} />
        <View>
          <Text preset="bodyStrong">Pro controller</Text>
          <Text preset="caption">Authenticated command broker</Text>
        </View>
      </View>
      <View style={$connectionCard}>
        <View style={$liveDot} />
        <View>
          <Text preset="bodyStrong">Convex state plane</Text>
          <Text preset="caption" numberOfLines={1}>
            {session.phase === "ready" ? session.bootstrap.convexUrl : "Unavailable"}
          </Text>
        </View>
      </View>
    </View>
  );
};

export const ControllerNewTaskScreen = () => (
  <View style={$sheet}>
    <Text preset="display">New task</Text>
    <Text preset="body" color={colors.textDim}>
      Task creation enters the durable command lane in the next work-object phase. This frame does
      not invent a second executor.
    </Text>
  </View>
);

export const ControllerSurfaceScreen = ({
  title,
  description,
}: {
  readonly title: string;
  readonly description: string;
}) => (
  <View style={$sheet}>
    <Text preset="display">{title}</Text>
    <Text preset="body" color={colors.textDim}>
      {description}
    </Text>
  </View>
);

export const ControllerSarahVoiceScreen = ({
  route,
  navigation,
}: NativeStackScreenProps<ControllerRouteParams, "SarahVoice">) => (
  <SarahVoiceScreen
    desktopThreadRef={route.params.desktopThreadRef}
    onClose={() => navigation.goBack()}
  />
);

const $root: ViewStyle = { flex: 1, backgroundColor: colors.background };
const $grow: ViewStyle = { flex: 1 };
const $outline: ViewStyle = {
  minHeight: 28,
  paddingHorizontal: spacing.medium,
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.tiny,
  borderBottomWidth: 1,
  borderBottomColor: colors.borderQuiet,
};
const $outlineMarker: ViewStyle = {
  minWidth: 20,
  minHeight: 20,
  alignItems: "center",
  justifyContent: "center",
};
const $adaptiveRow: ViewStyle = { flex: 1, flexDirection: "row" };
const $workspaceHeader: ViewStyle = {
  minHeight: 64,
  paddingHorizontal: spacing.medium,
  borderBottomWidth: 1,
  borderBottomColor: colors.border,
  backgroundColor: colors.surfaceSunken,
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "space-between",
};
const $workspaceIdentity: ViewStyle = {
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.small,
  flex: 1,
};
const $workspaceMark: ViewStyle = {
  width: 40,
  height: 40,
  borderRadius: radius.medium,
  alignItems: "center",
  justifyContent: "center",
  borderWidth: 1,
  borderColor: colors.borderEnergized,
  backgroundColor: colors.accentGlow,
};
const $sidebar: ViewStyle = { flex: 1, backgroundColor: colors.surfaceSunken };
const $sidebarTitle: ViewStyle = {
  padding: spacing.medium,
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "space-between",
  gap: spacing.small,
};
const $headerActions: ViewStyle = { flexDirection: "row", alignItems: "center" };
const $inboxContent: ViewStyle = { paddingHorizontal: spacing.small, paddingBottom: spacing.large };
const $inboxRow: ViewStyle = {
  minHeight: 76,
  borderBottomWidth: 1,
  borderBottomColor: colors.borderQuiet,
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.small,
  paddingHorizontal: spacing.small,
  paddingVertical: spacing.small,
};
const $selectedRow: ViewStyle = { backgroundColor: colors.accentGlow };
const $rowPressed: ViewStyle = { backgroundColor: colors.surface };
const $attentionRail: ViewStyle = { width: 3, alignSelf: "stretch", borderRadius: radius.pill };
const $rowBetween: ViewStyle = { flexDirection: "row", alignItems: "center", gap: spacing.small };
const $rowMeta: ViewStyle = { flexDirection: "row", gap: spacing.small, marginTop: spacing.tiny };
const $center: ViewStyle = {
  flex: 1,
  alignItems: "center",
  justifyContent: "center",
  gap: spacing.small,
};
const $empty: ViewStyle = { padding: spacing.large, gap: spacing.small };
const $splitDetail: ViewStyle = {
  flex: 1,
  borderLeftWidth: 1,
  borderLeftColor: colors.border,
  alignSelf: "stretch",
};
const $emptySelection: ViewStyle = {
  flex: 1,
  alignItems: "center",
  justifyContent: "center",
  padding: spacing.extraLarge,
  gap: spacing.small,
};
const $thread: ViewStyle = { flex: 1, backgroundColor: colors.background };
const $threadHeader: ViewStyle = {
  minHeight: 68,
  paddingHorizontal: spacing.medium,
  paddingVertical: spacing.small,
  borderBottomWidth: 1,
  borderBottomColor: colors.border,
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.small,
};
const $surfaceActions: ViewStyle = { flexDirection: "row", alignItems: "center" };
const $feedContent: ViewStyle = {
  padding: spacing.medium,
  gap: spacing.small,
  flexGrow: 1,
  justifyContent: "flex-end",
};
const $agentTurn: ViewStyle = { gap: spacing.tiny, paddingVertical: spacing.small };
const $userTurn: ViewStyle = {
  alignSelf: "flex-end",
  maxWidth: "88%",
  borderWidth: 1,
  borderColor: colors.borderEnergized,
  backgroundColor: colors.accentGlow,
  borderRadius: radius.medium,
  padding: spacing.small,
  gap: spacing.tiny,
};
const $composerRegion: ViewStyle = {
  borderTopWidth: 1,
  borderTopColor: colors.border,
  backgroundColor: colors.surfaceSunken,
  padding: spacing.small,
  gap: spacing.extraSmall,
};
const $composer: ViewStyle = {
  minHeight: 52,
  flexDirection: "row",
  alignItems: "flex-end",
  gap: spacing.small,
  borderWidth: 1,
  borderColor: colors.border,
  borderRadius: radius.large,
  backgroundColor: colors.surface,
  padding: spacing.extraSmall,
};
const $composerExpanded: ViewStyle = { minHeight: 104 };
const $composerInput: TextStyle = {
  flex: 1,
  minHeight: 44,
  maxHeight: 160,
  paddingHorizontal: spacing.extraSmall,
  paddingVertical: spacing.small,
  color: colors.text,
  fontFamily: typography.sans,
  fontSize: 16,
};
const $requestCard: ViewStyle = {
  borderWidth: 1,
  borderColor: colors.warn,
  backgroundColor: colors.warnGlow,
  borderRadius: radius.large,
  padding: spacing.medium,
  gap: spacing.small,
};
const $requestActions: ViewStyle = { flexDirection: "row", gap: spacing.small };
const $inputAnswer: ViewStyle = { gap: spacing.small };
const $input: TextStyle = {
  minHeight: 72,
  borderWidth: 1,
  borderColor: colors.border,
  borderRadius: radius.medium,
  padding: spacing.small,
  color: colors.text,
  fontFamily: typography.sans,
  fontSize: 16,
};
const $sheet: ViewStyle = {
  flex: 1,
  backgroundColor: colors.background,
  padding: spacing.large,
  gap: spacing.medium,
};
const $connectionCard: ViewStyle = {
  minHeight: 64,
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.small,
  borderWidth: 1,
  borderColor: colors.border,
  borderRadius: radius.medium,
  padding: spacing.medium,
};
const $liveDot: ViewStyle = {
  width: 10,
  height: 10,
  borderRadius: radius.pill,
  backgroundColor: colors.live,
};
