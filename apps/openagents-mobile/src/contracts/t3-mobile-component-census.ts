export type T3MobileComponentCensusRow = Readonly<{
  area: "shell" | "navigation" | "transcript" | "runtime" | "composer" | "workbench" | "native_finish"
  component: string
  packet: "A1" | "A2" | "A3" | "A4" | "B1" | "B2" | "C1" | "C2" | "D1" | "D2" | "E1" | "E2" | "F1" | "F2"
  implementation: "complete" | "adapted"
  evidence: string
}>

/**
 * Complete named-component census from the pinned T3 mobile gap analysis.
 * `adapted` means the T3 interaction is intentionally expressed through an
 * OpenAgents authority boundary (for example safe external web opening rather
 * than an embedded arbitrary webview), not that the component is absent.
 */
export const t3MobileComponentCensus: ReadonlyArray<T3MobileComponentCensusRow> = [
  { area: "shell", component: "Top navigation", packet: "C2", implementation: "complete", evidence: "screens/home-core.ts" },
  { area: "shell", component: "Phone navigation", packet: "C2", implementation: "complete", evidence: "screens/home-core.ts" },
  { area: "shell", component: "Tablet adaptive panes", packet: "C2", implementation: "complete", evidence: "screens/mobile-adaptive-workspace.ts" },
  { area: "shell", component: "Empty, loading, unavailable, and retry routes", packet: "F2", implementation: "complete", evidence: "screens/home-core.ts" },
  { area: "shell", component: "Hardware keyboard and system entry", packet: "C2", implementation: "complete", evidence: "screens/mobile-workspace-keyboard.ts" },
  { area: "navigation", component: "Project-aware thread and session rows", packet: "C1", implementation: "complete", evidence: "screens/mobile-workspace-navigation.ts" },
  { area: "navigation", component: "Project hierarchy", packet: "C1", implementation: "complete", evidence: "screens/mobile-workspace-navigation.ts" },
  { area: "navigation", component: "Search and filters", packet: "C1", implementation: "complete", evidence: "screens/home-core.ts" },
  { area: "navigation", component: "Thread lifecycle and archived navigation", packet: "C2", implementation: "complete", evidence: "screens/home-core.ts" },
  { area: "navigation", component: "Attention and causal jumps", packet: "C1", implementation: "complete", evidence: "attention/native-attention-target-delivery.ts" },
  { area: "navigation", component: "Controller directory and session inspector", packet: "F1", implementation: "complete", evidence: "coding/mobile-controller-directory.ts" },
  { area: "transcript", component: "Thread identity", packet: "C2", implementation: "complete", evidence: "screens/home-core.ts" },
  { area: "transcript", component: "User messages and actions", packet: "A4", implementation: "complete", evidence: "screens/khala-core.ts" },
  { area: "transcript", component: "Rich assistant messages", packet: "A1", implementation: "complete", evidence: "screens/mobile-transcript-content.ts" },
  { area: "transcript", component: "Grouped runtime work", packet: "A2", implementation: "complete", evidence: "screens/mobile-work-log.ts" },
  { area: "transcript", component: "Stable streaming presentation", packet: "A4", implementation: "complete", evidence: "screens/khala-core.ts" },
  { area: "transcript", component: "Content-specific message actions", packet: "A1", implementation: "complete", evidence: "screens/mobile-transcript-content.ts" },
  { area: "transcript", component: "Attachment cards and viewer", packet: "A4", implementation: "complete", evidence: "screens/mobile-transcript-attachment.ts" },
  { area: "transcript", component: "Pagination, unread, and anchor retention", packet: "A4", implementation: "complete", evidence: "screens/mobile-transcript-history.ts" },
  { area: "runtime", component: "Approval request card", packet: "A3", implementation: "complete", evidence: "screens/mobile-interaction-card.ts" },
  { area: "runtime", component: "Provider input card", packet: "A3", implementation: "complete", evidence: "screens/mobile-interaction-card.ts" },
  { area: "runtime", component: "Plan review card", packet: "A3", implementation: "complete", evidence: "screens/mobile-interaction-card.ts" },
  { area: "runtime", component: "Run controls", packet: "B2", implementation: "complete", evidence: "screens/mobile-composer-run-control.ts" },
  { area: "runtime", component: "Agent graph inspector", packet: "F1", implementation: "complete", evidence: "screens/khala-core.ts" },
  { area: "composer", component: "Collapsed composer shell", packet: "B1", implementation: "complete", evidence: "screens/khala-core.ts" },
  { area: "composer", component: "Expanded composer toolbar", packet: "B1", implementation: "complete", evidence: "screens/mobile-composer-toolbar.ts" },
  { area: "composer", component: "Grouped target, provider, and model picker", packet: "B1", implementation: "complete", evidence: "screens/mobile-composer-toolbar.ts" },
  { area: "composer", component: "Typed slash commands", packet: "B2", implementation: "complete", evidence: "screens/mobile-composer-discovery.ts" },
  { area: "composer", component: "Repository-backed path context", packet: "B2", implementation: "complete", evidence: "coding/mobile-composer-path-context.ts" },
  { area: "composer", component: "Attachment preview, remove, and retry", packet: "B2", implementation: "complete", evidence: "screens/mobile-composer-attachments.ts" },
  { area: "composer", component: "Queue, stop, and active-run behavior", packet: "B2", implementation: "complete", evidence: "conversation/mobile-runtime-queue.ts" },
  { area: "workbench", component: "Files tree and previews", packet: "D1", implementation: "complete", evidence: "screens/mobile-files-view.ts" },
  { area: "workbench", component: "Changes and inline review", packet: "D2", implementation: "complete", evidence: "screens/mobile-changes-view.ts" },
  { area: "workbench", component: "Git status, branch, commit, and push", packet: "E1", implementation: "complete", evidence: "screens/mobile-git-view.ts" },
  { area: "workbench", component: "Terminal sessions and replay", packet: "E2", implementation: "complete", evidence: "effect-native/mobile-terminal-host-driver.ts" },
  { area: "workbench", component: "Preview, artifact, and receipt inspection", packet: "D2", implementation: "adapted", evidence: "screens/mobile-files-view.ts" },
  { area: "native_finish", component: "Settings hierarchy", packet: "F1", implementation: "complete", evidence: "screens/mobile-settings-view.ts" },
  { area: "native_finish", component: "Connection pairing and health", packet: "F1", implementation: "complete", evidence: "settings/mobile-settings.ts" },
  { area: "native_finish", component: "Notification permission, preferences, and health", packet: "F1", implementation: "complete", evidence: "settings/expo-mobile-notification-settings.ts" },
  { area: "native_finish", component: "Share intake", packet: "F1", implementation: "complete", evidence: "settings/mobile-settings.ts" },
  { area: "native_finish", component: "Motion and haptic feedback", packet: "F2", implementation: "complete", evidence: "effect-native/mobile-native-feedback.ts" },
  { area: "native_finish", component: "Accessibility and focus behavior", packet: "F2", implementation: "complete", evidence: "screens/khala-core.ts" },
  { area: "native_finish", component: "OpenAgents visual system", packet: "F2", implementation: "adapted", evidence: "@effect-native/tokens" },
]
