import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { useQuery } from "convex/react";
import * as Crypto from "expo-crypto";
import { useCallback, useMemo, useReducer, useRef, useState } from "react";
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
import { Button } from "../ui/button";
import { Text } from "../ui/text";
import { colors, radius, spacing, typography } from "../ui/theme";
import {
  decodeAttentionInbox,
  decodeWorkDetails,
  decodeWorkShell,
  type AttentionShell,
  type ControllerTarget,
  type WorkDetail,
} from "./contracts";
import { attentionInboxQuery, workDetailsQuery, workShellQuery } from "./convex-functions";
import { initialFeedAnchorState, reduceFeedAnchor, shouldMaintainFeedEnd } from "./feed-anchor";
import { controllerLayout } from "./layout";
import type { ControllerRouteParams } from "./routes";
import { useControllerSession } from "./session-provider";
import { createSubmissionGuard } from "./submission-guard";

type HomeProps = NativeStackScreenProps<ControllerRouteParams, "Home">;
type ThreadProps = NativeStackScreenProps<ControllerRouteParams, "Thread">;

const payloadObject = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

const payloadText = (detail: WorkDetail): string => {
  const payload = payloadObject(detail.payload);
  for (const key of ["text", "summary", "message", "reason"]) {
    if (typeof payload[key] === "string") return payload[key];
  }
  return detail.kind.replaceAll(".", " ");
};

const formatAge = (timestamp: number): string => {
  const seconds = Math.max(0, Math.floor((Date.now() - timestamp) / 1_000));
  if (seconds < 60) return "now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `${hours}h` : `${Math.floor(hours / 24)}d`;
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

const DetailRow = ({ detail }: { readonly detail: WorkDetail }) => {
  const user = detail.kind === "message.user" || payloadObject(detail.payload).role === "user";
  const request = detail.kind === "approval.request" || detail.kind === "input.request";
  if (request) return null;
  return (
    <View style={user ? $userTurn : $agentTurn}>
      <Text preset="label" color={user ? colors.accentInk : colors.textFaint}>
        {user ? "YOU" : detail.kind.replaceAll(".", " ").toUpperCase()}
      </Text>
      <Text preset={user ? "mono" : "body"}>{payloadText(detail)}</Text>
      <Text preset="label" color={colors.textFaint}>
        {`${detail.state} · ${formatAge(detail.updatedAt)}`}
      </Text>
    </View>
  );
};

const PendingRequestCard = ({
  detail,
  busy,
  onRespond,
  onDisclosure,
}: {
  readonly detail: WorkDetail;
  readonly busy: boolean;
  readonly onRespond: (response: string | boolean) => void;
  readonly onDisclosure: (open: boolean) => void;
}) => {
  const approval = detail.kind === "approval.request";
  const [answer, setAnswer] = useState("");
  return (
    <View style={$requestCard} accessibilityRole="alert">
      <Text preset="label" color={colors.warn}>
        {approval ? "APPROVAL REQUIRED" : "INPUT REQUIRED"}
      </Text>
      <Text preset="bodyStrong">{payloadText(detail)}</Text>
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
  decisionRevision,
  shellLive,
  onDisclosure,
}: {
  readonly target: ControllerTarget;
  readonly running: boolean;
  readonly pendingRequest: WorkDetail | null;
  readonly decisionRevision: string;
  readonly shellLive: boolean;
  readonly onDisclosure: (open: boolean) => void;
}) => {
  const session = useControllerSession();
  const [draft, setDraft] = useState("");
  const [expanded, setExpanded] = useState(false);
  const [phase, setPhase] = useState<"idle" | "submitting">("idle");
  const [notice, setNotice] = useState<string | null>(null);
  const submissionGuard = useRef(createSubmissionGuard());

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
          pendingRequest.kind === "approval.request" ? "approval.respond" : "input.respond";
        const summary = await session.enqueueAndDrain(
          {
            commandId: `cmd_mobile_${Crypto.randomUUID()}`,
            operation,
            orderingKey: `${target.aggregateType}:${target.aggregateId}`,
            payload: { ...target, requestId: pendingRequest.detailId, response },
            createdAtMs: Date.now(),
            decisionRevision,
            expiresAtMs: Date.now() + 5 * 60_000,
          },
          {
            shellLive,
            decisionRevisions: { [operation]: decisionRevision },
          },
        );
        setNotice(summary.delivered > 0 ? "Response received." : "Response queued for delivery.");
      });
    },
    [decisionRevision, guarded, pendingRequest, session, shellLive, target],
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
          payload: { ...target, text },
          createdAtMs: Date.now(),
        },
        { shellLive, decisionRevisions: {} },
      );
      setDraft("");
      setExpanded(false);
      setNotice(
        summary.delivered > 0 ? "Message queued." : "Saved offline. It will send when live.",
      );
    });
  }, [draft, guarded, session, shellLive, target]);

  const stop = useCallback(() => {
    void guarded(async () => {
      const receipt = await session.interrupt(`cmd_mobile_${Crypto.randomUUID()}`, target);
      setNotice(receipt.status === "rejected" ? receipt.detail : "Interrupt requested.");
    });
  }, [guarded, session, target]);

  if (pendingRequest !== null) {
    return (
      <View style={$composerRegion}>
        <PendingRequestCard
          detail={pendingRequest}
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
          label={running ? "Stop" : "Send"}
          preset={running ? "danger" : "primary"}
          disabled={phase === "submitting" || (!running && draft.trim() === "")}
          onPress={running ? stop : send}
        />
      </View>
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
  const detailsRaw = useQuery(workDetailsQuery, {
    aggregateType: target.aggregateType,
    aggregateId: target.aggregateId,
    limit: 200,
  });
  const shell = useMemo(
    () => (shellRaw === undefined ? undefined : decodeWorkShell(shellRaw)),
    [shellRaw],
  );
  const details = useMemo(
    () => (detailsRaw === undefined ? [] : decodeWorkDetails(detailsRaw)),
    [detailsRaw],
  );
  const sortedDetails = useMemo(
    () => details.toSorted((left, right) => left.recordedAt - right.recordedAt),
    [details],
  );
  const pendingRequest = useMemo(() => {
    for (let index = sortedDetails.length - 1; index >= 0; index -= 1) {
      const detail = sortedDetails[index];
      if (
        detail !== undefined &&
        detail.state === "pending" &&
        (detail.kind === "approval.request" || detail.kind === "input.request")
      ) {
        return detail;
      }
    }
    return null;
  }, [sortedDetails]);
  const listRef = useRef<FlatList<WorkDetail>>(null);
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

  const renderDetail = useCallback(
    ({ item }: ListRenderItemInfo<WorkDetail>) => <DetailRow detail={item} />,
    [],
  );
  const running = shell?.status === "working" || shell?.status === "queued";
  const targetWithGeneration: ControllerTarget = {
    aggregateType: target.aggregateType,
    aggregateId: target.aggregateId,
    ...(shell === null || shell === undefined ? {} : { expectedGeneration: shell.generation }),
  };
  const decisionRevision =
    pendingRequest === null ? "none" : `${pendingRequest.detailId}:${pendingRequest.updatedAt}`;

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
      {detailsRaw === undefined ? (
        <View style={$center}>
          <ActivityIndicator color={colors.accent} />
        </View>
      ) : (
        <FlatList
          ref={listRef}
          data={sortedDetails}
          renderItem={renderDetail}
          keyExtractor={(item) => item.detailId}
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
        decisionRevision={decisionRevision}
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
