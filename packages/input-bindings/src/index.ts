import { Schema as S } from "effect"

export const OpenAgentsInputBindingSchemaVersion = S.Literal(
  "openagents.input-bindings.v1",
)
export type OpenAgentsInputBindingSchemaVersion =
  typeof OpenAgentsInputBindingSchemaVersion.Type

export const OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION: OpenAgentsInputBindingSchemaVersion =
  "openagents.input-bindings.v1"

export const OpenAgentsInputContext = S.Literals([
  "global",
  "text_entry",
  "command_palette",
  "verse_explore",
  "verse_code_overlay",
  "verse_pointer_locked",
  "managed_pane",
  "terminal",
])
export type OpenAgentsInputContext = typeof OpenAgentsInputContext.Type

export const openAgentsInputContexts: ReadonlyArray<OpenAgentsInputContext> = [
  "global",
  "text_entry",
  "command_palette",
  "verse_explore",
  "verse_code_overlay",
  "verse_pointer_locked",
  "managed_pane",
  "terminal",
]

export const OpenAgentsInputActionKind = S.Literals([
  "press",
  "hold",
  "toggle",
  "axis",
])
export type OpenAgentsInputActionKind = typeof OpenAgentsInputActionKind.Type

export const OpenAgentsInputModifiers = S.Struct({
  primary: S.optional(S.Boolean),
  shift: S.optional(S.Boolean),
  alt: S.optional(S.Boolean),
  ctrl: S.optional(S.Boolean),
  meta: S.optional(S.Boolean),
})
export type OpenAgentsInputModifiers = typeof OpenAgentsInputModifiers.Type

export const OpenAgentsKeyboardCodeBinding = S.Struct({
  type: S.Literal("keyboard_code"),
  code: S.String,
  modifiers: S.optional(OpenAgentsInputModifiers),
})
export type OpenAgentsKeyboardCodeBinding =
  typeof OpenAgentsKeyboardCodeBinding.Type

export const OpenAgentsKeyboardKeyBinding = S.Struct({
  type: S.Literal("keyboard_key"),
  key: S.String,
  modifiers: S.optional(OpenAgentsInputModifiers),
})
export type OpenAgentsKeyboardKeyBinding = typeof OpenAgentsKeyboardKeyBinding.Type

export const OpenAgentsMouseButtonBinding = S.Struct({
  type: S.Literal("mouse_button"),
  button: S.Number,
  modifiers: S.optional(OpenAgentsInputModifiers),
})
export type OpenAgentsMouseButtonBinding = typeof OpenAgentsMouseButtonBinding.Type

export const OpenAgentsWheelBinding = S.Struct({
  type: S.Literal("wheel"),
  direction: S.Literals(["up", "down"]),
  modifiers: S.optional(OpenAgentsInputModifiers),
})
export type OpenAgentsWheelBinding = typeof OpenAgentsWheelBinding.Type

export const OpenAgentsInputBinding = S.Union([
  OpenAgentsKeyboardCodeBinding,
  OpenAgentsKeyboardKeyBinding,
  OpenAgentsMouseButtonBinding,
  OpenAgentsWheelBinding,
])
export type OpenAgentsInputBinding = typeof OpenAgentsInputBinding.Type

export const OpenAgentsInputActionSpec = S.Struct({
  id: S.String,
  title: S.String,
  category: S.String,
  kind: OpenAgentsInputActionKind,
  contexts: S.Array(OpenAgentsInputContext),
  defaultBindings: S.Array(OpenAgentsInputBinding),
  reserved: S.optional(S.Boolean),
  editable: S.optional(S.Boolean),
})
export type OpenAgentsInputActionSpec = typeof OpenAgentsInputActionSpec.Type

export const OpenAgentsInputProfile = S.Struct({
  schemaVersion: OpenAgentsInputBindingSchemaVersion,
  profileId: S.String,
  bindings: S.Record(S.String, S.Array(OpenAgentsInputBinding)),
})
export type OpenAgentsInputProfile = typeof OpenAgentsInputProfile.Type

export const decodeOpenAgentsInputProfile = S.decodeUnknownSync(
  OpenAgentsInputProfile,
)

const binding = {
  code: (
    code: string,
    modifiers?: OpenAgentsInputModifiers,
  ): OpenAgentsInputBinding =>
    modifiers === undefined
      ? { type: "keyboard_code", code }
      : { type: "keyboard_code", code, modifiers },
  key: (
    key: string,
    modifiers?: OpenAgentsInputModifiers,
  ): OpenAgentsInputBinding =>
    modifiers === undefined
      ? { type: "keyboard_key", key }
      : { type: "keyboard_key", key, modifiers },
  mouse: (
    button: number,
    modifiers?: OpenAgentsInputModifiers,
  ): OpenAgentsInputBinding =>
    modifiers === undefined
      ? { type: "mouse_button", button }
      : { type: "mouse_button", button, modifiers },
  wheel: (
    direction: "up" | "down",
    modifiers?: OpenAgentsInputModifiers,
  ): OpenAgentsInputBinding =>
    modifiers === undefined
      ? { type: "wheel", direction }
      : { type: "wheel", direction, modifiers },
} as const

const action = (
  spec: OpenAgentsInputActionSpec,
): OpenAgentsInputActionSpec => spec

export const openAgentsInputActionSpecs: ReadonlyArray<OpenAgentsInputActionSpec> = [
  action({
    id: "movement.forward",
    title: "Move Forward",
    category: "Movement",
    kind: "hold",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("KeyW"), binding.code("ArrowUp")],
  }),
  action({
    id: "movement.backward",
    title: "Move Backward",
    category: "Movement",
    kind: "hold",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("KeyS"), binding.code("ArrowDown")],
  }),
  action({
    id: "movement.strafe_left",
    title: "Strafe Left",
    category: "Movement",
    kind: "hold",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("KeyA"), binding.code("ArrowLeft")],
  }),
  action({
    id: "movement.strafe_right",
    title: "Strafe Right",
    category: "Movement",
    kind: "hold",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("KeyD"), binding.code("ArrowRight")],
  }),
  action({
    id: "movement.sprint",
    title: "Sprint",
    category: "Movement",
    kind: "hold",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("ShiftLeft"), binding.code("ShiftRight")],
  }),
  action({
    id: "movement.jump",
    title: "Jump",
    category: "Movement",
    kind: "press",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("Space")],
  }),
  action({
    id: "movement.fall",
    title: "Fall",
    category: "Movement",
    kind: "hold",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("KeyC")],
  }),
  action({
    id: "movement.autorun",
    title: "Autorun",
    category: "Movement",
    kind: "toggle",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [],
  }),
  action({
    id: "camera.orbit_drag",
    title: "Orbit Camera",
    category: "Camera",
    kind: "hold",
    contexts: ["verse_explore", "verse_code_overlay"],
    defaultBindings: [binding.mouse(0)],
  }),
  action({
    id: "camera.zoom_in",
    title: "Zoom In",
    category: "Camera",
    kind: "press",
    contexts: ["verse_explore", "verse_code_overlay"],
    defaultBindings: [binding.wheel("up")],
  }),
  action({
    id: "camera.zoom_out",
    title: "Zoom Out",
    category: "Camera",
    kind: "press",
    contexts: ["verse_explore", "verse_code_overlay"],
    defaultBindings: [binding.wheel("down")],
  }),
  action({
    id: "camera.reset",
    title: "Reset Camera",
    category: "Camera",
    kind: "press",
    contexts: ["verse_explore", "verse_code_overlay"],
    defaultBindings: [],
  }),
  action({
    id: "target.next",
    title: "Target Next",
    category: "Targeting",
    kind: "press",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("Tab")],
  }),
  action({
    id: "target.previous",
    title: "Target Previous",
    category: "Targeting",
    kind: "press",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("Tab", { shift: true })],
  }),
  action({
    id: "target.nearest_pylon",
    title: "Target Nearest Pylon",
    category: "Targeting",
    kind: "press",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [],
  }),
  action({
    id: "target.nearest_avatar",
    title: "Target Nearest Avatar",
    category: "Targeting",
    kind: "press",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [],
  }),
  action({
    id: "target.clear",
    title: "Clear Target",
    category: "Targeting",
    kind: "press",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("Escape")],
  }),
  action({
    id: "interact.primary",
    title: "Interact",
    category: "Interaction",
    kind: "press",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("KeyE")],
  }),
  action({
    id: "interact.selected_info",
    title: "Open Selected Info",
    category: "Interaction",
    kind: "press",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("KeyF")],
  }),
  action({
    id: "interact.open_bulletin",
    title: "Open Bulletin",
    category: "Interaction",
    kind: "press",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [binding.code("KeyB")],
  }),
  action({
    id: "interact.pylon_actions",
    title: "Open Pylon Actions",
    category: "Interaction",
    kind: "press",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [],
  }),
  action({
    id: "tip.selected_pylon",
    title: "Tip Selected Pylon",
    category: "Interaction",
    kind: "press",
    contexts: ["verse_explore", "verse_pointer_locked"],
    defaultBindings: [],
  }),
  action({
    id: "tip.selected_forum_post",
    title: "Tip Selected Forum Post",
    category: "Interaction",
    kind: "press",
    contexts: ["verse_code_overlay", "managed_pane"],
    defaultBindings: [],
  }),
  action({
    id: "hud.toggle_code_overlay",
    title: "Toggle Code Overlay",
    category: "HUD",
    kind: "toggle",
    contexts: ["global", "verse_explore", "verse_code_overlay"],
    defaultBindings: [binding.key("v", { primary: true, shift: true })],
  }),
  action({
    id: "hud.toggle_diagnostics",
    title: "Toggle Diagnostics",
    category: "HUD",
    kind: "toggle",
    contexts: ["verse_code_overlay", "managed_pane"],
    defaultBindings: [],
  }),
  action({
    id: "hud.toggle_tassadar",
    title: "Toggle Tassadar HUD",
    category: "HUD",
    kind: "toggle",
    contexts: ["verse_explore", "verse_code_overlay"],
    defaultBindings: [],
  }),
  action({
    id: "hud.toggle_sats",
    title: "Toggle Sats HUD",
    category: "HUD",
    kind: "toggle",
    contexts: ["verse_explore", "verse_code_overlay"],
    defaultBindings: [],
  }),
  action({
    id: "app.command_palette",
    title: "Command Palette",
    category: "App",
    kind: "press",
    contexts: ["global", "managed_pane"],
    defaultBindings: [binding.key("k", { primary: true })],
  }),
  action({
    id: "app.submit",
    title: "Submit",
    category: "App",
    kind: "press",
    contexts: ["managed_pane", "text_entry"],
    defaultBindings: [binding.key("Enter", { primary: true })],
  }),
  action({
    id: "app.pane_next",
    title: "Next Pane",
    category: "App",
    kind: "press",
    contexts: ["managed_pane"],
    defaultBindings: [binding.key("j")],
  }),
  action({
    id: "app.pane_previous",
    title: "Previous Pane",
    category: "App",
    kind: "press",
    contexts: ["managed_pane"],
    defaultBindings: [binding.key("k")],
  }),
  action({
    id: "app.open_settings",
    title: "Open Settings",
    category: "App",
    kind: "press",
    contexts: ["global", "managed_pane"],
    defaultBindings: [],
  }),
  action({
    id: "app.focus_chat",
    title: "Focus Chat",
    category: "App",
    kind: "press",
    contexts: ["verse_explore", "verse_code_overlay", "managed_pane"],
    defaultBindings: [],
  }),
  action({
    id: "code.focus_composer",
    title: "Focus Composer",
    category: "Code",
    kind: "press",
    contexts: ["verse_code_overlay", "managed_pane"],
    defaultBindings: [],
  }),
  action({
    id: "code.new_thread",
    title: "New Coding Thread",
    category: "Code",
    kind: "press",
    contexts: ["verse_code_overlay", "managed_pane"],
    defaultBindings: [],
  }),
  action({
    id: "code.cancel_session",
    title: "Cancel Session",
    category: "Code",
    kind: "press",
    contexts: ["verse_code_overlay", "managed_pane"],
    defaultBindings: [],
  }),
  action({
    id: "code.toggle_accounts",
    title: "Toggle Accounts",
    category: "Code",
    kind: "press",
    contexts: ["verse_code_overlay", "managed_pane"],
    defaultBindings: [],
  }),
  action({
    id: "palette.close",
    title: "Close Palette",
    category: "App",
    kind: "press",
    contexts: ["command_palette"],
    defaultBindings: [binding.key("Escape")],
  }),
  action({
    id: "palette.run",
    title: "Run Palette Command",
    category: "App",
    kind: "press",
    contexts: ["command_palette"],
    defaultBindings: [binding.key("Enter")],
  }),
  action({
    id: "palette.move_up",
    title: "Palette Up",
    category: "App",
    kind: "press",
    contexts: ["command_palette"],
    defaultBindings: [binding.key("ArrowUp")],
  }),
  action({
    id: "palette.move_down",
    title: "Palette Down",
    category: "App",
    kind: "press",
    contexts: ["command_palette"],
    defaultBindings: [binding.key("ArrowDown")],
  }),
  ...Array.from({ length: 10 }, (_, index) =>
    action({
      id: `action_bar.slot_${index + 1}`,
      title: index === 0 ? "New Coder Session" : `Action Slot ${index + 1}`,
      category: "Action Bar",
      kind: "press",
      contexts: ["verse_explore", "verse_pointer_locked"],
      defaultBindings: [
        binding.code(index === 9 ? "Digit0" : `Digit${index + 1}`),
      ],
    }),
  ),
]

export const openAgentsInputActionSpecById = new Map(
  openAgentsInputActionSpecs.map((spec) => [spec.id, spec]),
)

export const openAgentsDefaultInputBindings: Readonly<
  Record<string, ReadonlyArray<OpenAgentsInputBinding>>
> = Object.fromEntries(
  openAgentsInputActionSpecs.map((spec) => [
    spec.id,
    spec.defaultBindings.map((defaultBinding) => ({ ...defaultBinding })),
  ]),
)

export const openAgentsDefaultInputProfile: OpenAgentsInputProfile = {
  schemaVersion: OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
  profileId: "default",
  bindings: openAgentsDefaultInputBindings,
}

export type OpenAgentsInputActionMap = Readonly<
  Record<string, ReadonlyArray<OpenAgentsInputBinding>>
>

export const openAgentsInputActionMapFromProfile = (
  profile: OpenAgentsInputProfile,
): OpenAgentsInputActionMap => normalizeOpenAgentsInputProfile(profile).bindings

export const normalizeOpenAgentsInputProfile = (
  profile: OpenAgentsInputProfile,
): OpenAgentsInputProfile => ({
  ...openAgentsDefaultInputProfile,
  ...profile,
  schemaVersion: OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
  bindings: {
    ...openAgentsDefaultInputProfile.bindings,
    ...profile.bindings,
  },
})

export const parseOpenAgentsInputProfileOrDefault = (
  value: unknown,
): OpenAgentsInputProfile => {
  try {
    return normalizeOpenAgentsInputProfile(decodeOpenAgentsInputProfile(value))
  } catch {
    return openAgentsDefaultInputProfile
  }
}

export type OpenAgentsInputConflictSeverity = "hard" | "reserved"

export type OpenAgentsInputConflict = Readonly<{
  severity: OpenAgentsInputConflictSeverity
  bindingKey: string
  bindingLabel: string
  actionIds: ReadonlyArray<string>
  contexts: ReadonlyArray<OpenAgentsInputContext>
  reason: string
}>

export const openAgentsNativeReservedBindings: ReadonlyArray<{
  binding: OpenAgentsInputBinding
  contexts: ReadonlyArray<OpenAgentsInputContext>
  reason: string
}> = [
  {
    binding: binding.key("q", { primary: true }),
    contexts: ["global"],
    reason: "Reserved for native app quit",
  },
  {
    binding: binding.key("h", { primary: true }),
    contexts: ["global"],
    reason: "Reserved for native app hide",
  },
  {
    binding: binding.key("m", { primary: true }),
    contexts: ["global"],
    reason: "Reserved for native app minimize",
  },
  {
    binding: binding.key("c", { primary: true }),
    contexts: ["text_entry", "terminal"],
    reason: "Reserved for native text copy",
  },
  {
    binding: binding.key("v", { primary: true }),
    contexts: ["text_entry", "terminal"],
    reason: "Reserved for native text paste",
  },
  {
    binding: binding.key("x", { primary: true }),
    contexts: ["text_entry", "terminal"],
    reason: "Reserved for native text cut",
  },
  {
    binding: binding.key("a", { primary: true }),
    contexts: ["text_entry", "terminal"],
    reason: "Reserved for native select all",
  },
  {
    binding: binding.key("z", { primary: true }),
    contexts: ["text_entry", "terminal"],
    reason: "Reserved for native undo",
  },
]

export const openAgentsInputBindingKey = (
  inputBinding: OpenAgentsInputBinding,
): string => {
  const modifiers = normalizeModifiers(inputBinding.modifiers)
  const modifierKey = [
    modifiers.primary ? "primary" : "",
    modifiers.ctrl ? "ctrl" : "",
    modifiers.meta ? "meta" : "",
    modifiers.alt ? "alt" : "",
    modifiers.shift ? "shift" : "",
  ].filter(Boolean).join("+")
  const prefix = modifierKey.length > 0 ? `${modifierKey}+` : ""

  switch (inputBinding.type) {
    case "keyboard_code":
      return `${prefix}code:${inputBinding.code}`
    case "keyboard_key":
      return `${prefix}key:${inputBinding.key.toLocaleLowerCase()}`
    case "mouse_button":
      return `${prefix}mouse:${inputBinding.button}`
    case "wheel":
      return `${prefix}wheel:${inputBinding.direction}`
  }
}

export const openAgentsInputBindingLabel = (
  inputBinding: OpenAgentsInputBinding,
): string => {
  const modifiers = normalizeModifiers(inputBinding.modifiers)
  const modifierLabels = [
    modifiers.primary ? "Cmd/Ctrl" : "",
    modifiers.ctrl ? "Ctrl" : "",
    modifiers.meta ? "Meta" : "",
    modifiers.alt ? "Alt" : "",
    modifiers.shift ? "Shift" : "",
  ].filter(Boolean)

  const mainLabel = (() => {
    switch (inputBinding.type) {
      case "keyboard_code":
        return keyboardCodeLabel(inputBinding.code)
      case "keyboard_key":
        return keyboardKeyLabel(inputBinding.key)
      case "mouse_button":
        return inputBinding.button === 0
          ? "Left Mouse"
          : inputBinding.button === 1
            ? "Middle Mouse"
            : inputBinding.button === 2
              ? "Right Mouse"
              : `Mouse ${inputBinding.button}`
      case "wheel":
        return inputBinding.direction === "up" ? "Wheel Up" : "Wheel Down"
    }
  })()

  return [...modifierLabels, mainLabel].join("+")
}

export const detectOpenAgentsInputConflicts = (
  profile: OpenAgentsInputProfile,
  specs: ReadonlyArray<OpenAgentsInputActionSpec> = openAgentsInputActionSpecs,
): ReadonlyArray<OpenAgentsInputConflict> => {
  const normalizedProfile = normalizeOpenAgentsInputProfile(profile)
  const conflicts: Array<OpenAgentsInputConflict> = []
  const entriesByBinding = new Map<
    string,
    Array<{
      actionId: string
      binding: OpenAgentsInputBinding
      contexts: ReadonlyArray<OpenAgentsInputContext>
    }>
  >()

  for (const spec of specs) {
    const bindings = normalizedProfile.bindings[spec.id] ?? []
    for (const profileBinding of bindings) {
      const key = openAgentsInputBindingKey(profileBinding)
      const entries = entriesByBinding.get(key) ?? []
      entries.push({
        actionId: spec.id,
        binding: profileBinding,
        contexts: spec.contexts,
      })
      entriesByBinding.set(key, entries)
    }
  }

  for (const [bindingKey, entries] of entriesByBinding) {
    for (let leftIndex = 0; leftIndex < entries.length; leftIndex += 1) {
      for (
        let rightIndex = leftIndex + 1;
        rightIndex < entries.length;
        rightIndex += 1
      ) {
        const left = entries[leftIndex]
        const right = entries[rightIndex]
        const overlappingContexts = intersectContexts(
          left.contexts,
          right.contexts,
        )
        if (overlappingContexts.length === 0) {
          continue
        }
        conflicts.push({
          severity: "hard",
          bindingKey,
          bindingLabel: openAgentsInputBindingLabel(left.binding),
          actionIds: [left.actionId, right.actionId],
          contexts: overlappingContexts,
          reason: "Two actions use the same binding in the same context",
        })
      }
    }
  }

  for (const spec of specs) {
    const bindings = normalizedProfile.bindings[spec.id] ?? []
    for (const profileBinding of bindings) {
      const profileBindingKey = openAgentsInputBindingKey(profileBinding)
      for (const reserved of openAgentsNativeReservedBindings) {
        const reservedBindingKey = openAgentsInputBindingKey(reserved.binding)
        if (profileBindingKey !== reservedBindingKey) {
          continue
        }
        const overlappingContexts = intersectContexts(
          spec.contexts,
          reserved.contexts,
        )
        if (overlappingContexts.length === 0) {
          continue
        }
        conflicts.push({
          severity: "reserved",
          bindingKey: profileBindingKey,
          bindingLabel: openAgentsInputBindingLabel(profileBinding),
          actionIds: [spec.id],
          contexts: overlappingContexts,
          reason: reserved.reason,
        })
      }
    }
  }

  return conflicts
}

type NormalizedOpenAgentsInputModifiers = Readonly<{
  primary: boolean
  shift: boolean
  alt: boolean
  ctrl: boolean
  meta: boolean
}>

const normalizeModifiers = (
  modifiers: OpenAgentsInputModifiers | undefined,
): NormalizedOpenAgentsInputModifiers => ({
  primary: modifiers?.primary === true,
  shift: modifiers?.shift === true,
  alt: modifiers?.alt === true,
  ctrl: modifiers?.ctrl === true,
  meta: modifiers?.meta === true,
})

const keyboardCodeLabel = (code: string): string => {
  if (/^Key[A-Z]$/.test(code)) {
    return code.slice(3)
  }
  if (/^Digit[0-9]$/.test(code)) {
    return code.slice(5)
  }
  if (code === "Space") {
    return "Space"
  }
  if (code === "Tab") {
    return "Tab"
  }
  if (code === "Enter") {
    return "Enter"
  }
  if (code === "Escape") {
    return "Escape"
  }
  if (code === "ShiftLeft") {
    return "Left Shift"
  }
  if (code === "ShiftRight") {
    return "Right Shift"
  }
  if (code.startsWith("Arrow")) {
    return code.slice("Arrow".length)
  }
  return code
}

const keyboardKeyLabel = (key: string): string => {
  if (key.length === 1) {
    return key.toLocaleUpperCase()
  }
  if (key.startsWith("Arrow")) {
    return key.slice("Arrow".length)
  }
  return key
}

const intersectContexts = (
  left: ReadonlyArray<OpenAgentsInputContext>,
  right: ReadonlyArray<OpenAgentsInputContext>,
): ReadonlyArray<OpenAgentsInputContext> =>
  left.filter((context) => right.includes(context))

export type OpenAgentsKeyboardEventType = "keydown" | "keyup"

export type OpenAgentsKeyboardEventLike = Readonly<{
  code?: string
  key?: string
  repeat?: boolean
  shiftKey?: boolean
  altKey?: boolean
  ctrlKey?: boolean
  metaKey?: boolean
  preventDefault?: () => void
}>

export type OpenAgentsKeyboardEventListener = (
  event: OpenAgentsKeyboardEventLike,
) => void

export type OpenAgentsKeyboardEventSource = Readonly<{
  addEventListener: (
    type: OpenAgentsKeyboardEventType,
    listener: OpenAgentsKeyboardEventListener,
  ) => void
  removeEventListener: (
    type: OpenAgentsKeyboardEventType,
    listener: OpenAgentsKeyboardEventListener,
  ) => void
}>

export type OpenAgentsKeyboardControlsChange = Readonly<{
  actionId: string
  pressed: boolean
  source: OpenAgentsKeyboardEventType | "reset" | "bindings_updated"
  bindingKey: string | null
  bindingLabel: string | null
  repeat: boolean
}>

export type OpenAgentsKeyboardControlsSubscriber = (
  change: OpenAgentsKeyboardControlsChange,
  state: Readonly<Record<string, boolean>>,
) => void

export type OpenAgentsKeyboardControlsOptions = Readonly<{
  actionMap: OpenAgentsInputActionMap
  eventSource: OpenAgentsKeyboardEventSource
  allowExtraModifiers?: boolean
  preventDefault?: boolean | ((change: OpenAgentsKeyboardControlsChange) => boolean)
  onChange?: OpenAgentsKeyboardControlsSubscriber
}>

export type OpenAgentsKeyboardControls = Readonly<{
  getState: () => Readonly<Record<string, boolean>>
  isPressed: (actionId: string) => boolean
  subscribe: (subscriber: OpenAgentsKeyboardControlsSubscriber) => () => void
  updateBindings: (actionMap: OpenAgentsInputActionMap) => void
  reset: () => void
  dispose: () => void
}>

export const createOpenAgentsKeyboardControls = (
  options: OpenAgentsKeyboardControlsOptions,
): OpenAgentsKeyboardControls => {
  let actionMap = options.actionMap
  let disposed = false
  const allowExtraModifiers = options.allowExtraModifiers !== false
  const subscribers = new Set<OpenAgentsKeyboardControlsSubscriber>()
  const state: Record<string, boolean> = defaultKeyboardControlsState(actionMap)

  if (options.onChange !== undefined) {
    subscribers.add(options.onChange)
  }

  const emit = (change: OpenAgentsKeyboardControlsChange): void => {
    for (const subscriber of subscribers) {
      subscriber(change, { ...state })
    }
  }

  const setPressed = (
    actionId: string,
    pressed: boolean,
    source: OpenAgentsKeyboardControlsChange["source"],
    binding: OpenAgentsInputBinding | null,
    repeat: boolean,
  ): void => {
    if (state[actionId] === pressed) {
      return
    }
    state[actionId] = pressed
    emit({
      actionId,
      pressed,
      source,
      bindingKey: binding === null ? null : openAgentsInputBindingKey(binding),
      bindingLabel: binding === null ? null : openAgentsInputBindingLabel(binding),
      repeat,
    })
  }

  const handleKeyboardEvent = (
    type: OpenAgentsKeyboardEventType,
    event: OpenAgentsKeyboardEventLike,
  ): void => {
    if (disposed) {
      return
    }
    const matches = resolveOpenAgentsKeyboardEventActionBindings(
      actionMap,
      event,
      { allowExtraModifiers, release: type === "keyup" },
    )
    let shouldPreventDefault = false
    for (const match of matches) {
      const wasPressed = state[match.actionId] === true
      const isRepeat = event.repeat === true && wasPressed
      if (type === "keydown") {
        if (!isRepeat) {
          setPressed(match.actionId, true, type, match.binding, event.repeat === true)
        }
      } else {
        setPressed(match.actionId, false, type, match.binding, false)
      }
      const change: OpenAgentsKeyboardControlsChange = {
        actionId: match.actionId,
        pressed: type === "keydown",
        source: type,
        bindingKey: openAgentsInputBindingKey(match.binding),
        bindingLabel: openAgentsInputBindingLabel(match.binding),
        repeat: isRepeat,
      }
      shouldPreventDefault = shouldPreventDefault ||
        options.preventDefault === true ||
        (typeof options.preventDefault === "function" &&
          options.preventDefault(change))
    }
    if (shouldPreventDefault) {
      event.preventDefault?.()
    }
  }

  const keydownListener: OpenAgentsKeyboardEventListener = (event) => {
    handleKeyboardEvent("keydown", event)
  }
  const keyupListener: OpenAgentsKeyboardEventListener = (event) => {
    handleKeyboardEvent("keyup", event)
  }

  options.eventSource.addEventListener("keydown", keydownListener)
  options.eventSource.addEventListener("keyup", keyupListener)

  const resetState = (
    source: "reset" | "bindings_updated",
  ): void => {
    for (const actionId of Object.keys(state)) {
      setPressed(actionId, false, source, null, false)
    }
  }

  return {
    getState: () => ({ ...state }),
    isPressed: (actionId: string) => state[actionId] === true,
    subscribe: (subscriber: OpenAgentsKeyboardControlsSubscriber) => {
      subscribers.add(subscriber)
      return () => {
        subscribers.delete(subscriber)
      }
    },
    updateBindings: (nextActionMap: OpenAgentsInputActionMap) => {
      resetState("bindings_updated")
      actionMap = nextActionMap
      for (const key of Object.keys(state)) {
        delete state[key]
      }
      Object.assign(state, defaultKeyboardControlsState(nextActionMap))
    },
    reset: () => {
      resetState("reset")
    },
    dispose: () => {
      if (disposed) {
        return
      }
      resetState("reset")
      disposed = true
      subscribers.clear()
      options.eventSource.removeEventListener("keydown", keydownListener)
      options.eventSource.removeEventListener("keyup", keyupListener)
    },
  }
}

export const resolveOpenAgentsKeyboardEventActionBindings = (
  actionMap: OpenAgentsInputActionMap,
  event: OpenAgentsKeyboardEventLike,
  options?: Readonly<{ allowExtraModifiers?: boolean; release?: boolean }>,
): ReadonlyArray<Readonly<{ actionId: string; binding: OpenAgentsInputBinding }>> => {
  const matches: Array<Readonly<{ actionId: string; binding: OpenAgentsInputBinding }>> = []
  for (const [actionId, bindings] of Object.entries(actionMap)) {
    for (const candidateBinding of bindings) {
      if (
        openAgentsKeyboardEventMatchesBinding(candidateBinding, event, {
          allowExtraModifiers: options?.allowExtraModifiers !== false,
          release: options?.release === true,
        })
      ) {
        matches.push({ actionId, binding: candidateBinding })
      }
    }
  }
  return matches
}

export const openAgentsKeyboardEventMatchesBinding = (
  inputBinding: OpenAgentsInputBinding,
  event: OpenAgentsKeyboardEventLike,
  options?: Readonly<{ allowExtraModifiers?: boolean; release?: boolean }>,
): boolean => {
  if (inputBinding.type !== "keyboard_code" && inputBinding.type !== "keyboard_key") {
    return false
  }

  const baseMatches =
    inputBinding.type === "keyboard_code"
      ? event.code === inputBinding.code
      : event.key?.toLocaleLowerCase() === inputBinding.key.toLocaleLowerCase()

  if (!baseMatches) {
    return false
  }

  if (options?.release === true) {
    return true
  }

  return keyboardModifiersMatch(
    inputBinding.modifiers,
    event,
    options?.allowExtraModifiers !== false,
  )
}

const keyboardModifiersMatch = (
  modifiers: OpenAgentsInputModifiers | undefined,
  event: OpenAgentsKeyboardEventLike,
  allowExtraModifiers: boolean,
): boolean => {
  const expected = normalizeModifiers(modifiers)
  const actual = {
    primary: event.metaKey === true || event.ctrlKey === true,
    shift: event.shiftKey === true,
    alt: event.altKey === true,
    ctrl: event.ctrlKey === true,
    meta: event.metaKey === true,
  }

  if (expected.primary && !actual.primary) {
    return false
  }
  if (expected.shift && !actual.shift) {
    return false
  }
  if (expected.alt && !actual.alt) {
    return false
  }
  if (expected.ctrl && !actual.ctrl) {
    return false
  }
  if (expected.meta && !actual.meta) {
    return false
  }

  if (allowExtraModifiers) {
    return true
  }

  const ctrlAllowed = expected.ctrl || expected.primary
  const metaAllowed = expected.meta || expected.primary

  return (
    expected.shift === actual.shift &&
    expected.alt === actual.alt &&
    (!actual.ctrl || ctrlAllowed) &&
    (!actual.meta || metaAllowed) &&
    (expected.primary || expected.ctrl || expected.meta || !actual.primary)
  )
}

const defaultKeyboardControlsState = (
  actionMap: OpenAgentsInputActionMap,
): Record<string, boolean> =>
  Object.fromEntries(Object.keys(actionMap).map((actionId) => [actionId, false]))
