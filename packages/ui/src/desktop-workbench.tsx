/**
 * `@openagentsinc/ui/desktop-workbench` — public re-export barrel.
 *
 * The implementation is split into per-component modules under
 * `./workbench/*` (#8860, epic #8857 Wave 1) so parallel Wave-2 lanes
 * (T4-T12) can each add one module plus one dispatch branch without
 * colliding on a single file. This barrel exists only to keep the published
 * import path (`@openagentsinc/ui/desktop-workbench`, pinned in
 * `package.json` `exports`) stable for every consumer: the desktop renderer
 * and the web `/splash` route. Nothing here mounts a runtime, reads storage,
 * or owns app state — this stays presentation-only, consumed by whatever
 * app-shell/router the host provides.
 */
import "./desktop-workbench.css"

export { DesktopWorkbench, DesktopSidebarExpand, DesktopRailScrim } from "./workbench/frame.tsx"
export {
  DesktopSessionRail,
  type DesktopRailIcon,
  type DesktopRailDestination,
  type DesktopRailSession,
  type DesktopSessionRailProps,
} from "./workbench/rail.tsx"
export { desktopThemeCssVariables, type DesktopThemeCssVariables } from "./workbench/theme-bridge.ts"
export {
  DesktopConversationHeader,
  DesktopConversation,
  type DesktopConversationHeaderMeter,
} from "./workbench/header.tsx"
export {
  ContextMeter,
  type ContextMeterProps,
  type ContextMeterUsage,
  type ContextMeterRateLimitWindow,
} from "./workbench/context-meter.tsx"
export { DesktopTimeline } from "./workbench/timeline.tsx"
export { DesktopTimelineMessage } from "./workbench/message.tsx"
export { DesktopTimelineNotice } from "./workbench/notice.tsx"
export { DesktopWorkEntry, DesktopWorkGroup } from "./workbench/work-entry.tsx"
export { type DesktopActivityStatus } from "./workbench/activity-status.tsx"
export { DesktopPlanCard, type DesktopPlanEntry } from "./workbench/plan-card.tsx"
export { DesktopCommandCard } from "./workbench/command-card.tsx"
export { DesktopFileChangeCard, type DesktopFileChange } from "./workbench/file-change-card.tsx"
export {
  DesktopToolCallCard,
  type DesktopToolKind,
  type DesktopToolCallArg,
  type DesktopToolCallCardProps,
} from "./workbench/tool-call-card.tsx"
export {
  desktopToolCallCardFixtures,
  type DesktopToolCallCardFixture,
} from "./workbench/tool-call-card.fixtures.ts"
export {
  DesktopAgentGroup,
  type DesktopAgentStatus,
  type DesktopAgentActivity,
} from "./workbench/agent-group.tsx"
export { DesktopApprovalCard, type DesktopApprovalDecision } from "./workbench/approval-card.tsx"
export { DesktopQueuedFollowup } from "./workbench/queued-followup.tsx"
export {
  DesktopComposerFrame,
  DesktopComposerInput,
  DesktopComposerBar,
  DesktopComposerButton,
  type DesktopComposerButtonKind,
} from "./workbench/composer.tsx"

// Per-variant dispatch table (WorkbenchItem kind -> shared component). See
// `./workbench/dispatch.tsx` for the wiring status of every branch.
export {
  dispatchWorkbenchItem,
  type WorkbenchDispatchItem,
  type WorkbenchDispatchContext,
  type WorkbenchDispatchSource,
  type WorkbenchDispatchStatus,
  type WorkbenchMessageDispatchItem,
  type WorkbenchReasoningDispatchItem,
  type WorkbenchCommandDispatchItem,
  type WorkbenchFileChangeDispatchItem,
  type WorkbenchFileChangeEntryDispatch,
  type WorkbenchToolCallDispatchItem,
  type WorkbenchAgentDispatchItem,
  type WorkbenchPlanDispatchItem,
  type WorkbenchApprovalDispatchItem,
  type WorkbenchMeterDispatchItem,
  type WorkbenchNoticeDispatchItem,
  type WorkbenchCompactionDispatchItem,
  type WorkbenchSleepDispatchItem,
  type WorkbenchReviewDispatchItem,
  type WorkbenchHookDispatchItem,
} from "./workbench/dispatch.tsx"
