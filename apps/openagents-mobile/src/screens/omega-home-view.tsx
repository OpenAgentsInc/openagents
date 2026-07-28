import { useCallback, useMemo, useRef, type ReactNode } from "react";
import {
  ActivityIndicator,
  FlatList,
  View,
  type ListRenderItemInfo,
  type TextStyle,
  type ViewStyle,
} from "react-native";

import { Button } from "../ui/button";
import { Screen } from "../ui/screen";
import { Markdown, markdownToPlainText } from "../ui/markdown";
import {
  Badge,
  Card,
  Divider,
  EmptyState,
  Field,
  StatusDot,
  type BadgeTone,
} from "../ui/surfaces";
import { Text } from "../ui/text";
import { colors, radius, spacing } from "../ui/theme";
import type {
  OmegaDeviceBridgeState,
  OmegaMirrorRun,
  OmegaMirrorThread,
} from "../workroom/omega-device-bridge-client";

export type OmegaHomeActivity =
  | Readonly<{ type: "thread"; updatedAt: number; thread: OmegaMirrorThread }>
  | Readonly<{ type: "run"; updatedAt: number; run: OmegaMirrorRun }>;

export type OmegaHomeViewModel = Readonly<{
  desktopName: string;
  connectionLabel: string;
  connectionTone: BadgeTone;
  staleness: string;
  paired: boolean;
  notice: string | null;
  activity: ReadonlyArray<OmegaHomeActivity>;
  selectedThread: OmegaMirrorThread | null;
  threadDraft: string;
  commandLaneAvailable: boolean;
  commandNotice: string | null;
  /** The clock the relative stamps are read against, so rows agree. */
  now: number;
}>;

export type OmegaHomeActions = Readonly<{
  onPairPressed: () => void;
  onActivitySelected: (threadRef: string) => void;
  onThreadClosed: () => void;
  onDraftChanged: (next: string) => void;
  onEnqueuePressed: () => void;
  onSteerPressed: () => void;
}>;

export const connectionToneOf = (state: OmegaDeviceBridgeState): BadgeTone => {
  switch (state.connection.state) {
    case "direct":
      return "success";
    case "relay":
      return "info";
    case "offline":
      return "danger";
  }
};

/**
 * How long ago, in the width a list row can spare.
 *
 * Seconds round up to `1m` rather than showing a number that is stale by the
 * time it is read. Anything older than a week stops counting days, because a
 * three-digit day count says less than a date would.
 */
export const shortAgo = (at: number, now: number): string => {
  const seconds = Math.max(0, Math.round((now - at) / 1000));
  if (seconds < 60) return "now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d`;
  const weeks = Math.floor(days / 7);
  return `${weeks}w`;
};

const executorLine = (thread: OmegaMirrorThread): string => {
  const model = thread.executor.modelName ?? thread.executor.modelId;
  return model === null
    ? thread.executor.executorName
    : `${thread.executor.executorName} · ${model}`;
};

/**
 * The link state, and nothing else.
 *
 * The owner directed this down to one dot on 2026-07-27: the desktop name and
 * the freshness line were restating what the screen below already shows. A dot
 * still answers the only question the strip owes ("is this live?"), and its
 * accessibility label keeps the wording for a person who cannot see colour.
 */
const ConnectionHeader = ({
  model,
  leading,
}: {
  readonly model: OmegaHomeViewModel;
  readonly leading?: ReactNode;
}) => (
  <View style={$header}>
    <View style={$headerRow}>
      {/*
        The leading slot takes the remaining width so the dot stays pinned to
        the right edge whether or not anything sits beside it. A bare
        `space-between` put the dot on the LEFT on a screen with no leading
        content, because one child has nothing to be spaced against.
      */}
      <View style={$headerLeading}>{leading}</View>
      <StatusDot
        tone={model.connectionTone}
        accessibilityLabel={`${model.connectionLabel}. ${model.staleness}`}
      />
    </View>
    {model.notice === null ? null : (
      <Text preset="caption" color={colors.warn}>
        {model.notice}
      </Text>
    )}
  </View>
);

const ThreadRow = ({
  thread,
  now,
  onPress,
}: {
  readonly thread: OmegaMirrorThread;
  readonly now: number;
  readonly onPress: () => void;
}) => {
  const last = thread.transcript[thread.transcript.length - 1];
  const at = last?.createdAt ?? thread.updatedAt;
  const ago = shortAgo(at, now);
  return (
    <Card
      onPress={onPress}
      accessibilityLabel={`${thread.title}. ${executorLine(thread)}. ${thread.state}. Last activity ${ago}. Open thread.`}
      style={$row}
    >
      <View style={$rowHead}>
        <Text preset="subheading" numberOfLines={1} style={$rowTitle}>
          {thread.title}
        </Text>
        <View style={$rowMeta}>
          <Text preset="label" color={colors.textFaint}>
            {ago}
          </Text>
          <Badge label={thread.state} tone={thread.state === "running" ? "info" : "neutral"} />
        </View>
      </View>
      <Text preset="caption" numberOfLines={1}>
        {executorLine(thread)}
      </Text>
      {last === undefined ? null : (
        <Text preset="body" color={colors.textDim} numberOfLines={2} style={$rowPreview}>
          {markdownToPlainText(last.text)}
        </Text>
      )}
    </Card>
  );
};

const RunRow = ({ run, now }: { readonly run: OmegaMirrorRun; readonly now: number }) => {
  const ago = shortAgo(run.updatedAt, now);
  return (
    <Card
      accessibilityLabel={`${run.title}. Omega, ${run.lane}. ${run.state}. Last activity ${ago}.`}
      style={$row}
    >
      <View style={$rowHead}>
        <Text preset="subheading" numberOfLines={1} style={$rowTitle}>
          {run.title}
        </Text>
        <View style={$rowMeta}>
          <Text preset="label" color={colors.textFaint}>
            {ago}
          </Text>
          <Badge label={run.state} tone={run.state === "running" ? "info" : "neutral"} />
        </View>
      </View>
      <Text preset="caption">{`Omega · ${run.lane}`}</Text>
    </Card>
  );
};

const speakerOf = (role: "user" | "assistant" | "system" | "tool"): string => {
  switch (role) {
    case "user":
      return "YOU";
    case "assistant":
      return "OMEGA";
    case "system":
      return "SYSTEM";
    case "tool":
      return "TOOL";
  }
};

/**
 * A tool call, in the shape the desktop draws it.
 *
 * The desktop renders a tool call as its own card with a state marker on the
 * left, so a person reads "something is happening" before reading what. The
 * mirror sends `label — state`, which is split back apart here rather than
 * printed as a sentence.
 */
const ToolCard = ({ text }: { readonly text: string }) => {
  const split = text.lastIndexOf(" — ");
  const label = split === -1 ? text : text.slice(0, split);
  const state = split === -1 ? "" : text.slice(split + 3);
  const running = state === "running" || state === "queued";
  const failed = state === "failed" || state === "rejected" || state === "canceled";
  return (
    <View style={$toolCard}>
      {running ? (
        <ActivityIndicator size="small" color={colors.accent} style={$toolMark} />
      ) : (
        <Text preset="body" color={failed ? colors.fault : colors.live} style={$toolMark}>
          {failed ? "✕" : "✓"}
        </Text>
      )}
      <Text preset="body" numberOfLines={2} style={$toolLabel}>
        {label}
      </Text>
      {state === "" ? null : (
        <Text preset="label" color={running ? colors.accentInk : colors.textFaint}>
          {state}
        </Text>
      )}
    </View>
  );
};

/**
 * A transcript turn.
 *
 * The desktop puts a person's own words in a bordered mono box and lets the
 * agent's answer run as plain prose on the page. Following that here means the
 * two surfaces read as one product rather than as a chat app that happens to
 * show the same words.
 */
const TranscriptTurn = ({
  role,
  text,
}: {
  readonly role: "user" | "assistant" | "system" | "tool";
  readonly text: string;
}) => {
  if (role === "tool") {
    return <ToolCard text={text} />;
  }
  if (role === "user") {
    return (
      <View style={$personTurn}>
        <Text preset="mono" style={$personText}>
          {text}
        </Text>
      </View>
    );
  }
  // System turns carry failure reasons from the desktop callout. Paint them
  // as faults so a person on the phone sees the same problem the desktop
  // already named — not a silent "failed" subtitle with no body.
  if (role === "system") {
    return (
      <View style={$errorTurn} accessibilityRole="alert">
        <Text preset="label" color={colors.fault} style={$speaker}>
          ERROR
        </Text>
        <Text preset="body" color={colors.fault}>
          {text}
        </Text>
      </View>
    );
  }
  return (
    <View style={$turn}>
      <Text preset="label" color={colors.textFaint} style={$speaker}>
        {speakerOf(role)}
      </Text>
      <Markdown source={text} />
    </View>
  );
};

export const OmegaHomeView = ({
  model,
  actions,
}: {
  readonly model: OmegaHomeViewModel;
  readonly actions: OmegaHomeActions;
}) => {
  const listRef = useRef<FlatList<OmegaHomeActivity>>(null);

  const renderActivity = useCallback(
    ({ item }: ListRenderItemInfo<OmegaHomeActivity>) =>
      item.type === "thread" ? (
        <ThreadRow
          thread={item.thread}
          now={model.now}
          onPress={() => actions.onActivitySelected(item.thread.threadRef)}
        />
      ) : (
        <RunRow run={item.run} now={model.now} />
      ),
    [actions, model.now],
  );

  const activityKey = useCallback(
    (item: OmegaHomeActivity) =>
      item.type === "thread" ? `thread:${item.thread.threadRef}` : `run:${item.run.runRef}`,
    [],
  );

  const transcript = useMemo(
    () => model.selectedThread?.transcript ?? [],
    [model.selectedThread],
  );

  if (!model.paired) {
    return (
      <Screen>
        <ConnectionHeader model={model} />
        <View style={$pairing}>
          <Text preset="display">Mirror your desktop</Text>
          <Text preset="body" color={colors.textDim} style={$pairingBody}>
            Open Omega on your desktop, show its device code, and scan it here. The phone keeps
            only the connection grant and resume cursor.
          </Text>
          <Button label="Scan desktop QR" onPress={actions.onPairPressed} preset="primary" />
        </View>
      </Screen>
    );
  }

  if (model.selectedThread !== null) {
    const thread = model.selectedThread;
    return (
      <Screen keyboardOffset={0}>
        <ConnectionHeader
          model={model}
          leading={
            <Button
              label="← Activity"
              preset="ghost"
              onPress={actions.onThreadClosed}
              style={$backButton}
            />
          }
        />
        <View style={$threadHead}>
          <Text preset="heading" numberOfLines={2}>
            {thread.title}
          </Text>
          <View style={$threadMeta}>
            {thread.state === "running" ? (
              <ActivityIndicator size="small" color={colors.accent} />
            ) : null}
            <Text preset="caption" style={$threadMetaText}>
              {`${executorLine(thread)} · ${thread.state}`}
            </Text>
          </View>
        </View>
        <Divider />
        <FlatList
          data={transcript}
          keyExtractor={(message) => message.messageRef}
          renderItem={({ item }) => <TranscriptTurn role={item.role} text={item.text} />}
          contentContainerStyle={$transcript}
          ListEmptyComponent={
            <EmptyState
              heading="No messages yet"
              body="Messages appear here as the desktop thread runs."
            />
          }
          showsVerticalScrollIndicator={false}
          keyboardDismissMode="interactive"
        />
        {model.commandLaneAvailable ? (
          <View style={$composer}>
            <Field
              value={model.threadDraft}
              onChangeText={actions.onDraftChanged}
              placeholder="Send work to this desktop thread"
              accessibilityLabel="Message this desktop thread"
              multiline
            />
            {model.commandNotice === null ? null : (
              <Text preset="caption" color={colors.warn}>
                {model.commandNotice}
              </Text>
            )}
            <View style={$composerActions}>
              <Button
                label="Send"
                preset="primary"
                disabled={model.threadDraft.trim() === ""}
                onPress={actions.onEnqueuePressed}
              />
              <Button
                label="Steer"
                preset="secondary"
                disabled={model.threadDraft.trim() === ""}
                onPress={actions.onSteerPressed}
              />
            </View>
          </View>
        ) : null}
      </Screen>
    );
  }

  return (
    <Screen>
      <ConnectionHeader
        model={model}
        leading={
          <Text preset="label" color={colors.textDim} style={$headerLabel}>
            OMEGA SYNC
          </Text>
        }
      />
      <FlatList
        ref={listRef}
        data={model.activity}
        keyExtractor={activityKey}
        renderItem={renderActivity}
        contentContainerStyle={$feed}
        ItemSeparatorComponent={() => <View style={$gap} />}
        ListEmptyComponent={
          <EmptyState
            heading="No desktop activity yet"
            body="Threads and runs appear here when Omega reports them."
          />
        }
        showsVerticalScrollIndicator={false}
      />
    </Screen>
  );
};

const $header: ViewStyle = {
  paddingHorizontal: spacing.medium,
  paddingTop: spacing.extraSmall,
  paddingBottom: spacing.tiny,
  gap: spacing.tiny,
};

const $headerRow: ViewStyle = {
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.small,
  minHeight: 28,
};

const $headerLeading: ViewStyle = { flex: 1, justifyContent: "center" };

const $headerLabel: TextStyle = { letterSpacing: 1 };

// The back control sits in the header strip, so it drops the button's own
// vertical padding and keeps its label on one line.
const $backButton: ViewStyle = { minHeight: 0, paddingVertical: 0, paddingHorizontal: 0 };

const $pairing: ViewStyle = {
  flex: 1,
  justifyContent: "center",
  paddingHorizontal: spacing.medium,
  gap: spacing.medium,
};

const $pairingBody: TextStyle = { maxWidth: 340 };

const $feed: ViewStyle = {
  paddingHorizontal: spacing.medium,
  paddingTop: spacing.small,
  paddingBottom: spacing.extraLarge,
};


const $gap: ViewStyle = { height: spacing.small };

const $row: ViewStyle = { gap: spacing.tiny };

const $rowHead: ViewStyle = {
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "space-between",
  gap: spacing.small,
};

const $rowTitle: TextStyle = { flexShrink: 1 };

const $rowMeta: ViewStyle = { flexDirection: "row", alignItems: "center", gap: spacing.extraSmall };

const $rowPreview: TextStyle = { marginTop: spacing.micro };


const $threadMeta: ViewStyle = {
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.extraSmall,
};

const $threadMetaText: TextStyle = { flexShrink: 1 };

const $threadHead: ViewStyle = {
  paddingHorizontal: spacing.medium,
  paddingBottom: spacing.small,
  gap: spacing.micro,
};

const $transcript: ViewStyle = {
  paddingHorizontal: spacing.medium,
  paddingVertical: spacing.small,
  gap: spacing.small,
};

const $turn: ViewStyle = { gap: spacing.tiny, paddingVertical: spacing.extraSmall };

// Failure reasons mirrored from the desktop callout.
const $errorTurn: ViewStyle = {
  gap: spacing.tiny,
  paddingVertical: spacing.extraSmall,
  paddingHorizontal: spacing.small,
  borderWidth: 1,
  borderColor: colors.fault,
  borderRadius: radius.medium,
  backgroundColor: colors.surfaceSunken,
};

// The person's own words, in the bordered mono box the desktop composer uses.
const $personTurn: ViewStyle = {
  backgroundColor: colors.surfaceSunken,
  borderWidth: 1,
  borderColor: colors.border,
  borderRadius: radius.medium,
  paddingHorizontal: spacing.small,
  paddingVertical: spacing.extraSmall,
};

const $personText: TextStyle = { color: colors.text };

const $toolCard: ViewStyle = {
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.extraSmall,
  backgroundColor: colors.surface,
  borderWidth: 1,
  borderColor: colors.border,
  borderRadius: radius.medium,
  paddingHorizontal: spacing.small,
  paddingVertical: spacing.extraSmall,
};

const $toolMark: ViewStyle = { width: 16, alignItems: "center" };

const $toolLabel: TextStyle = { flex: 1, color: colors.text };
const $speaker: TextStyle = {};

const $composer: ViewStyle = {
  paddingHorizontal: spacing.medium,
  paddingTop: spacing.small,
  paddingBottom: spacing.small,
  borderTopWidth: 1,
  borderTopColor: colors.border,
  backgroundColor: colors.surface,
  gap: spacing.extraSmall,
};

const $composerActions: ViewStyle = { flexDirection: "row", gap: spacing.extraSmall };
