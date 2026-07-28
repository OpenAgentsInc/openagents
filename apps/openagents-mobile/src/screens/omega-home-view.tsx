import { useCallback, useMemo, useRef } from "react";
import {
  FlatList,
  View,
  type ListRenderItemInfo,
  type TextStyle,
  type ViewStyle,
} from "react-native";

import { Button } from "../ui/button";
import { Screen } from "../ui/screen";
import { Badge, Card, Divider, EmptyState, Field, type BadgeTone } from "../ui/surfaces";
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

const executorLine = (thread: OmegaMirrorThread): string => {
  const model = thread.executor.modelName ?? thread.executor.modelId;
  return model === null
    ? thread.executor.executorName
    : `${thread.executor.executorName} · ${model}`;
};

/** The header states which desktop this is and whether the link is live. */
const ConnectionHeader = ({ model }: { readonly model: OmegaHomeViewModel }) => (
  <View style={$header}>
    <View style={$headerRow}>
      <Text preset="heading" numberOfLines={1} style={$headerTitle}>
        {model.desktopName}
      </Text>
      <Badge label={model.connectionLabel} tone={model.connectionTone} />
    </View>
    <Text preset="caption">{model.staleness}</Text>
    {model.notice === null ? null : (
      <Text preset="caption" color={colors.warning}>
        {model.notice}
      </Text>
    )}
  </View>
);

const ThreadRow = ({
  thread,
  onPress,
}: {
  readonly thread: OmegaMirrorThread;
  readonly onPress: () => void;
}) => {
  const last = thread.transcript[thread.transcript.length - 1];
  return (
    <Card
      onPress={onPress}
      accessibilityLabel={`${thread.title}. ${executorLine(thread)}. ${thread.state}. Open thread.`}
      style={$row}
    >
      <View style={$rowHead}>
        <Text preset="subheading" numberOfLines={1} style={$rowTitle}>
          {thread.title}
        </Text>
        <Badge label={thread.state} tone={thread.state === "running" ? "info" : "neutral"} />
      </View>
      <Text preset="caption" numberOfLines={1}>
        {executorLine(thread)}
      </Text>
      {last === undefined ? null : (
        <Text preset="body" color={colors.textDim} numberOfLines={2} style={$rowPreview}>
          {last.text}
        </Text>
      )}
    </Card>
  );
};

const RunRow = ({ run }: { readonly run: OmegaMirrorRun }) => (
  <Card accessibilityLabel={`${run.title}. Omega, ${run.lane}. ${run.state}.`} style={$row}>
    <View style={$rowHead}>
      <Text preset="subheading" numberOfLines={1} style={$rowTitle}>
        {run.title}
      </Text>
      <Badge label={run.state} tone={run.state === "running" ? "info" : "neutral"} />
    </View>
    <Text preset="caption">{`Omega · ${run.lane}`}</Text>
  </Card>
);

/** A transcript message, the person's on a raised surface. */
const MessageBubble = ({
  role,
  text,
}: {
  readonly role: "user" | "assistant" | "system" | "tool";
  readonly text: string;
}) => (
  <View style={[$bubble, role === "user" ? $bubbleUser : $bubbleAgent]}>
    <Text preset="caption" color={role === "user" ? colors.tint : colors.textDim}>
      {role}
    </Text>
    <Text preset="body" style={$bubbleText}>
      {text}
    </Text>
  </View>
);

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
          onPress={() => actions.onActivitySelected(item.thread.threadRef)}
        />
      ) : (
        <RunRow run={item.run} />
      ),
    [actions],
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
        <ConnectionHeader model={model} />
        <View style={$threadBar}>
          <Button label="← Activity" preset="ghost" onPress={actions.onThreadClosed} />
        </View>
        <View style={$threadHead}>
          <Text preset="heading" numberOfLines={2}>
            {thread.title}
          </Text>
          <Text preset="caption">{`${executorLine(thread)} · ${thread.state}`}</Text>
        </View>
        <Divider />
        <FlatList
          data={transcript}
          keyExtractor={(message) => message.messageRef}
          renderItem={({ item }) => <MessageBubble role={item.role} text={item.text} />}
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
        <View style={$composer}>
          <Field
            value={model.threadDraft}
            onChangeText={actions.onDraftChanged}
            placeholder={
              model.commandLaneAvailable
                ? "Send work to this desktop thread"
                : "The signed command lane is unavailable"
            }
            accessibilityLabel="Message this desktop thread"
            multiline
          />
          {model.commandNotice === null ? null : (
            <Text preset="caption" color={colors.warning}>
              {model.commandNotice}
            </Text>
          )}
          <View style={$composerActions}>
            <Button
              label="Send"
              preset="primary"
              disabled={!model.commandLaneAvailable || model.threadDraft.trim() === ""}
              onPress={actions.onEnqueuePressed}
            />
            <Button
              label="Steer"
              preset="secondary"
              disabled={!model.commandLaneAvailable || model.threadDraft.trim() === ""}
              onPress={actions.onSteerPressed}
            />
          </View>
        </View>
      </Screen>
    );
  }

  return (
    <Screen>
      <ConnectionHeader model={model} />
      <FlatList
        ref={listRef}
        data={model.activity}
        keyExtractor={activityKey}
        renderItem={renderActivity}
        contentContainerStyle={$feed}
        ItemSeparatorComponent={() => <View style={$gap} />}
        ListHeaderComponent={
          <Text preset="label" color={colors.textDim} style={$feedHeading}>
            DESKTOP ACTIVITY
          </Text>
        }
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
  paddingVertical: spacing.small,
  backgroundColor: colors.surface,
  borderBottomWidth: 1,
  borderBottomColor: colors.separator,
  gap: spacing.tiny,
};

const $headerRow: ViewStyle = {
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "space-between",
  gap: spacing.small,
};

const $headerTitle: TextStyle = { flexShrink: 1 };

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

const $feedHeading: TextStyle = { marginBottom: spacing.small, letterSpacing: 1 };

const $gap: ViewStyle = { height: spacing.small };

const $row: ViewStyle = { gap: spacing.tiny };

const $rowHead: ViewStyle = {
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "space-between",
  gap: spacing.small,
};

const $rowTitle: TextStyle = { flexShrink: 1 };

const $rowPreview: TextStyle = { marginTop: spacing.micro };

const $threadBar: ViewStyle = { paddingHorizontal: spacing.extraSmall, paddingTop: spacing.tiny };

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

const $bubble: ViewStyle = {
  borderRadius: radius.medium,
  borderWidth: 1,
  borderColor: colors.separator,
  padding: spacing.small,
  gap: spacing.micro,
};

const $bubbleUser: ViewStyle = { backgroundColor: colors.surfaceRaised };
const $bubbleAgent: ViewStyle = { backgroundColor: colors.surface };
const $bubbleText: TextStyle = {};

const $composer: ViewStyle = {
  paddingHorizontal: spacing.medium,
  paddingTop: spacing.small,
  paddingBottom: spacing.small,
  borderTopWidth: 1,
  borderTopColor: colors.separator,
  backgroundColor: colors.surface,
  gap: spacing.extraSmall,
};

const $composerActions: ViewStyle = { flexDirection: "row", gap: spacing.extraSmall };
