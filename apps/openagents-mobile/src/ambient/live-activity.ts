import type { LiveActivityConfig, LiveActivityState } from "expo-live-activity";

import { liveActivitySubtitle, type LiveActivityShellProjection } from "./contracts";

export interface LiveActivityRuntime {
  readonly start: (
    state: LiveActivityState,
    config: LiveActivityConfig,
  ) => string | undefined | void;
  readonly update: (activityId: string, state: LiveActivityState) => void;
  readonly stop: (activityId: string, state: LiveActivityState) => void;
}

export type ReconciledLiveActivity = Readonly<{
  activityId: string;
  targetKey: string;
  generation: number;
}> | null;

const stateFor = (projection: LiveActivityShellProjection): LiveActivityState => ({
  title: "OpenAgents",
  subtitle: liveActivitySubtitle(projection),
});

const configFor = (projection: LiveActivityShellProjection): LiveActivityConfig => ({
  deepLinkUrl: `/work/${encodeURIComponent(projection.aggregateType)}/${encodeURIComponent(
    projection.aggregateId,
  )}?workspaceId=${encodeURIComponent(projection.workspaceId)}`,
});

const targetKey = (projection: LiveActivityShellProjection): string =>
  `${projection.workspaceId}:${projection.aggregateType}:${projection.aggregateId}`;

export const reconcileLiveActivity = (
  runtime: LiveActivityRuntime,
  current: ReconciledLiveActivity,
  projection: LiveActivityShellProjection | null,
): ReconciledLiveActivity => {
  if (projection === null) {
    if (current !== null) {
      runtime.stop(current.activityId, {
        title: "OpenAgents",
        subtitle: "Work is settled",
      });
    }
    return null;
  }

  const nextKey = targetKey(projection);
  if (current !== null && current.targetKey === nextKey) {
    if (projection.generation <= current.generation) return current;
    runtime.update(current.activityId, stateFor(projection));
    return { ...current, generation: projection.generation };
  }
  if (current !== null) {
    runtime.stop(current.activityId, {
      title: "OpenAgents",
      subtitle: "Status moved to another item",
    });
  }
  const activityId = runtime.start(stateFor(projection), configFor(projection));
  return activityId === undefined
    ? null
    : { activityId, targetKey: nextKey, generation: projection.generation };
};
