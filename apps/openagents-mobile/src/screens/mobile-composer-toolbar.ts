import {
  Badge,
  Button,
  ComponentValueBinding,
  IntentRef,
  Sheet,
  Stack,
  StaticPayload,
  Text,
  TextField,
  type View,
} from "@effect-native/core"

import type { MobileCodingComposerSession } from "../coding/mobile-coding-composer"
import type {
  MobileExecutionTargetOption,
  MobileExecutionTargetReadiness,
} from "../coding/mobile-execution-targets"
import type { MobileAccessibilityProfile } from "./khala-core"

export interface MobileComposerToolbarState {
  readonly pickerOpen: boolean
  readonly search: string
}

const modelLabel = (modelRef: string): string =>
  modelRef.replace(/^model\./u, "").replaceAll("-", " ")

const readinessCopy = (
  readiness: MobileExecutionTargetReadiness,
  reasonRef: string | undefined,
): string => {
  switch (readiness) {
    case "ready": return "Ready"
    case "revoked": return "Sign in again to use this account"
    case "offline": return "This execution target is offline"
    case "unavailable":
      switch (reasonRef) {
        case "reason.account_exhausted": return "Usage limit reached"
        case "reason.account_rate_limited": return "Temporarily rate limited"
        case "reason.auto_unresolved": return "Automatic target could not be resolved"
        case "reason.target_not_advertised": return "No longer advertised by this environment"
        default: return "Unavailable"
      }
  }
}

export const groupedMobileExecutionTargets = (
  targets: ReadonlyArray<MobileExecutionTargetOption>,
  search: string,
): ReadonlyArray<Readonly<{
  providerLabel: MobileExecutionTargetOption["providerLabel"]
  options: ReadonlyArray<MobileExecutionTargetOption>
}>> => {
  const query = search.trim().toLocaleLowerCase()
  const filtered = query === ""
    ? targets
    : targets.filter(target => [
        target.label,
        target.providerLabel,
        target.modelRef,
        target.readiness,
      ].some(value => value.toLocaleLowerCase().includes(query)))
  const order: ReadonlyArray<MobileExecutionTargetOption["providerLabel"]> = [
    "OpenAgents",
    "Codex",
    "Claude",
  ]
  return order.flatMap(providerLabel => {
    const options = filtered.filter(target => target.providerLabel === providerLabel)
    return options.length === 0 ? [] : [{ providerLabel, options }]
  })
}

export const renderMobileComposerToolbar = (
  session: MobileCodingComposerSession,
  targets: ReadonlyArray<MobileExecutionTargetOption>,
  state: MobileComposerToolbarState,
  accessibility: MobileAccessibilityProfile,
  attachmentPicking = false,
  attachmentStatus: Readonly<{ kind: "ready" | "failed"; message: string }> | null = null,
): ReadonlyArray<View> => {
  const selected = targets.find(target =>
    target.targetId === session.draft.target.executionTargetRef)
  const groups = groupedMobileExecutionTargets(targets, state.search)
  const currentLabel = selected === undefined
    ? session.targetLabel
    : `${selected.label} · ${modelLabel(selected.modelRef)}`
  return [
    Stack({
      key: "khala-coding-composer-toolbar",
      direction: "column",
      gap: "1",
      style: { width: "full", padding: "1", backgroundColor: "surface", borderRadius: "lg" },
      a11y: { role: "group", label: "Coding composer controls" },
    }, [
      Text({
        key: "khala-coding-composer-location",
        content: `${session.repositoryLabel} · ${session.worktreeLabel}`,
        variant: "caption",
        color: "textMuted",
      }),
      Stack({ key: "khala-coding-composer-toolbar-actions", direction: "row", gap: "1", align: "center", style: { width: "full" } }, [
        Button({
          key: "khala-coding-composer-target-picker",
          label: targets.length === 0 ? "Target unavailable" : currentLabel,
          variant: "ghost",
          disabled: targets.length === 0,
          onPress: IntentRef("CodingComposerTargetPickerOpened", StaticPayload({})),
          a11y: { label: `Execution target and model. ${currentLabel}. Open picker.` },
          style: { flex: 1, minHeight: accessibility.minTouchTarget },
        }),
        Badge({
          key: "khala-coding-composer-mode",
          label: "Code",
          tone: "neutral",
          a11y: { role: "presentation", label: "Composer mode, Code" },
        }),
      ]),
      ...(targets.length === 0
        ? [Text({
            key: "khala-coding-target-catalog-unavailable",
            content: "Execution targets are unavailable. Your draft is preserved.",
            variant: "caption",
            color: "warning",
          })]
        : []),
      ...(attachmentPicking
        ? [Text({
            key: "khala-coding-composer-attachment-picking",
            content: "Choosing files or images…",
            variant: "caption",
            color: "textMuted",
          })]
        : attachmentStatus === null
          ? []
          : [Text({
              key: "khala-coding-composer-attachment-status",
              content: attachmentStatus.message,
              variant: "caption",
              color: attachmentStatus.kind === "failed" ? "danger" : "textMuted",
            })]),
    ]),
    Sheet({
      key: "khala-coding-target-picker-sheet",
      open: state.pickerOpen,
      dismissable: true,
      edge: "bottom",
      detents: ["full"],
      presentationDetents: ["full"],
      onDismiss: IntentRef("CodingComposerTargetPickerDismissed", StaticPayload({})),
      a11y: { role: "dialog", label: "Choose execution target and model" },
    }, [
      Text({
        key: "khala-coding-target-picker-title",
        content: "Execution target",
        variant: "title",
        color: "textPrimary",
      }),
      Text({
        key: "khala-coding-target-picker-mode-note",
        content: "Code mode · Model and account follow the selected authoritative target.",
        variant: "caption",
        color: "textMuted",
      }),
      TextField({
        key: "khala-coding-target-picker-search",
        value: state.search,
        label: "Search targets",
        placeholder: "Provider, model, or account",
        onChange: IntentRef("CodingComposerTargetSearchChanged", ComponentValueBinding()),
        variant: "outline",
        size: "md",
        style: { width: "full" },
      }),
      ...(targets.length === 0
        ? [Text({
            key: "khala-coding-target-picker-no-catalog",
            content: "No authenticated execution-target catalog is available. Close this picker and reconnect.",
            variant: "body",
            color: "warning",
          })]
        : groups.length === 0
          ? [Text({
              key: "khala-coding-target-picker-empty-search",
              content: `No targets match “${state.search.trim()}”.`,
              variant: "body",
              color: "textMuted",
            })]
          : groups.flatMap(group => [
              Text({
                key: `khala-coding-target-group-${group.providerLabel}`,
                content: group.providerLabel,
                variant: "heading",
                color: "textPrimary",
              }),
              ...group.options.map(option => {
                const isSelected = option.targetId === session.draft.target.executionTargetRef
                const readiness = readinessCopy(option.readiness, option.reasonRef)
                return Stack({
                  key: `khala-coding-target-row-${option.targetId}`,
                  direction: "column",
                  gap: "0.5",
                  style: { width: "full" },
                }, [
                  Button({
                    key: `khala-coding-target-${option.targetId}`,
                    label: option.label,
                    variant: isSelected ? "secondary" : "ghost",
                    selected: isSelected,
                    disabled: option.readiness !== "ready",
                    onPress: IntentRef("CodingExecutionTargetSelected", StaticPayload({ targetId: option.targetId })),
                    a11y: { label: `${option.accessibilityLabel}. Model ${modelLabel(option.modelRef)}. ${readiness}${isSelected ? ". Selected" : ""}` },
                    style: { width: "full", minHeight: accessibility.minTouchTarget },
                  }),
                  Text({
                    key: `khala-coding-target-${option.targetId}-detail`,
                    content: `${modelLabel(option.modelRef)} · ${readiness}`,
                    variant: "caption",
                    color: option.readiness === "ready" ? "textMuted" : "warning",
                  }),
                ])
              }),
            ])),
    ]),
  ]
}
