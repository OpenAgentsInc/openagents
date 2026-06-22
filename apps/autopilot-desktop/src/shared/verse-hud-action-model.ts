export type VerseHotbarActionKind =
  | "chat"
  | "focus"
  | "inspect"
  | "new_coder_session"
  | "tip"

export type VerseHotbarAction = Readonly<{
  actionId: string
  kind: VerseHotbarActionKind
  label: string
  iconName?: string
  slot?: number
  targetRef?: string
}>

export type VerseHotbarSlot = Readonly<{
  action: VerseHotbarAction | null
  actionId: string
  enabled: boolean
  key: string
  slot: number
}>

export type VerseChatChannel = "forum" | "global" | "local" | "run"

export type VerseChatDraftProjection = Readonly<{
  channel: VerseChatChannel
  text: string
  prefix: string
}>

export type VerseWorldModeration = Readonly<{
  state: "blocked" | "masked" | "muted" | "visible"
  replacementText?: string
  sourceRefs?: ReadonlyArray<string>
}>

export type VerseModerationDisplay = Readonly<{
  visible: boolean
  text: string
  tone: "blocked" | "masked" | "muted" | "visible"
  sourceRefs: ReadonlyArray<string>
}>

export type VerseContextTarget = Readonly<{
  ref: string
  kind: "avatar" | "pylon"
  label: string
  online?: boolean
}>

export type VerseContextAction = Readonly<{
  actionId: string
  label: string
  targetRef: string
  enabled: boolean
}>

const defaultHotbarActions: ReadonlyArray<VerseHotbarAction> = [
  {
    actionId: "new-coder-session",
    kind: "new_coder_session",
    label: "New Coder Session",
    iconName: "OpenaiLogoRegular",
    slot: 1,
  },
]

export const projectVerseHotbarSlots = (input: {
  readonly actions?: ReadonlyArray<VerseHotbarAction>
  readonly slotCount?: number
}): ReadonlyArray<VerseHotbarSlot> => {
  const slotCount = Math.max(1, Math.min(10, Math.floor(input.slotCount ?? 10)))
  const slots: Array<VerseHotbarAction | null> = Array.from({ length: slotCount }, () => null)
  const seen = new Set<string>()
  const queue = [...defaultHotbarActions, ...(input.actions ?? [])]

  for (const action of queue) {
    if (seen.has(action.actionId)) continue
    seen.add(action.actionId)
    const preferred = action.slot === undefined ? -1 : action.slot - 1
    const index =
      preferred >= 0 && preferred < slotCount && slots[preferred] === null
        ? preferred
        : slots.findIndex(slot => slot === null)
    if (index >= 0) slots[index] = action
  }

  return slots.map((action, index) => ({
    action,
    actionId: action?.actionId ?? `empty.${index + 1}`,
    enabled: action !== null,
    key: index === 9 ? "0" : String(index + 1),
    slot: index + 1,
  }))
}

const channelPrefixes: Readonly<Record<string, VerseChatChannel>> = {
  "/f": "forum",
  "/forum": "forum",
  "/g": "global",
  "/global": "global",
  "/l": "local",
  "/local": "local",
  "/r": "run",
  "/run": "run",
}

export const projectVerseChatDraft = (
  raw: string,
  fallbackChannel: VerseChatChannel = "local",
): VerseChatDraftProjection => {
  const trimmed = raw.trim()
  const [head = "", ...rest] = trimmed.split(/\s+/)
  const channel = channelPrefixes[head.toLowerCase()] ?? fallbackChannel
  const text = channelPrefixes[head.toLowerCase()] === undefined
    ? trimmed
    : rest.join(" ").trim()
  const prefix =
    channel === "forum" ? "/forum"
      : channel === "global" ? "/global"
        : channel === "run" ? "/run"
          : "/local"
  return { channel, text, prefix }
}

export const verseChatTimestampLabel = (
  iso: string,
  now: Date = new Date(iso),
): string => {
  const date = new Date(iso)
  if (!Number.isFinite(date.getTime())) return "time unknown"
  const sameDay = date.toISOString().slice(0, 10) === now.toISOString().slice(0, 10)
  return sameDay
    ? date.toISOString().slice(11, 16)
    : `${date.toISOString().slice(5, 10)} ${date.toISOString().slice(11, 16)}`
}

export const projectVerseModerationDisplay = (input: {
  readonly text: string
  readonly moderation: VerseWorldModeration
}): VerseModerationDisplay => {
  const sourceRefs = input.moderation.sourceRefs ?? []
  if (input.moderation.state === "blocked") {
    return { visible: false, text: "", tone: "blocked", sourceRefs }
  }
  if (input.moderation.state === "muted") {
    return { visible: false, text: "", tone: "muted", sourceRefs }
  }
  if (input.moderation.state === "masked") {
    return {
      visible: true,
      text: input.moderation.replacementText ?? "[moderated]",
      tone: "masked",
      sourceRefs,
    }
  }
  return { visible: true, text: input.text, tone: "visible", sourceRefs }
}

export const verseContextActionsForTarget = (
  target: VerseContextTarget,
): ReadonlyArray<VerseContextAction> => {
  const online = target.online !== false
  const base: Array<VerseContextAction> = [
    {
      actionId: `inspect:${target.ref}`,
      label: `Inspect ${target.label}`,
      targetRef: target.ref,
      enabled: true,
    },
    {
      actionId: `focus:${target.ref}`,
      label: `Focus ${target.label}`,
      targetRef: target.ref,
      enabled: true,
    },
  ]
  if (target.kind === "pylon") {
    base.push({
      actionId: `tip:${target.ref}`,
      label: `Tip ${target.label}`,
      targetRef: target.ref,
      enabled: online,
    })
  } else {
    base.push({
      actionId: `chat:${target.ref}`,
      label: `Chat with ${target.label}`,
      targetRef: target.ref,
      enabled: online,
    })
  }
  return base
}
