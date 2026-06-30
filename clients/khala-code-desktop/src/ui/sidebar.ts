import { Schema } from "effect"
import { Runtime } from "foldkit"
import type { Command } from "foldkit"
import {
  sidebarInit,
  sidebarUpdate,
  sidebarView,
  type SidebarPrimitiveMessage,
  type SidebarPrimitiveModel,
  type SidebarViewGroup,
} from "@openagentsinc/ui/basecoat/sidebar"

// The sidebar runs as a small, self-contained Foldkit/Effect island mounted into
// a dedicated container in the Khala Code Desktop shell. Its state is a Foldkit
// Model, interactions flow through SidebarPrimitiveMessage + sidebarUpdate, and
// it renders through sidebarView. No manual DOM, no class-name shim.

export type SidebarNavMessage = SidebarPrimitiveMessage

type SidebarModel = SidebarPrimitiveModel

type SidebarCommands = ReadonlyArray<Command<SidebarNavMessage, never, never>>

const NO_COMMANDS: SidebarCommands = []

// Foldkit requires a Schema codec for the Model (devtools / freeze model).
const SidebarPrimitiveItemSchema = Schema.Struct({
  value: Schema.String,
  disabled: Schema.optional(Schema.Boolean),
})

const SidebarModelSchema: Schema.Codec<SidebarModel, unknown, unknown, unknown> = Schema.Struct({
  open: Schema.Boolean,
  breakpoint: Schema.Number,
  items: Schema.Array(SidebarPrimitiveItemSchema),
  focusedValue: Schema.NullOr(Schema.String),
  selectedValue: Schema.NullOr(Schema.String),
})

// The desktop has no session-list RPC yet, so the initial sidebar is a small,
// representative navigation. The app owns the nav data + current selection; the
// sidebar owns chrome + behavior. Replace these groups when a session list lands.
const navGroups = (): ReadonlyArray<SidebarViewGroup<SidebarNavMessage>> => [
  {
    label: "Khala Code",
    items: [
      { value: "chat", children: ["Chat"] },
      { value: "sessions", children: ["Sessions"] },
    ],
  },
  {
    label: "Fleet",
    items: [
      { value: "fleet", children: ["Fleet status"] },
      { value: "settings", children: ["Settings"] },
    ],
  },
]

const navItems = (): ReadonlyArray<{ value: string }> =>
  navGroups().flatMap(group => group.items.map(item => ({ value: item.value })))

export type SidebarMountOptions = Readonly<{
  readonly selectedValue?: string | null
  readonly onActivate?: (value: string) => void
}>

export const mountKhalaCodeSidebar = (
  container: HTMLElement,
  options: SidebarMountOptions = {},
): void => {
  const init = (): readonly [SidebarModel, SidebarCommands] => [
    sidebarInit({
      items: navItems(),
      initialOpen: true,
      viewportWidth: window.innerWidth,
      ...(options.selectedValue === undefined || options.selectedValue === null
        ? {}
        : { selectedValue: options.selectedValue }),
    }),
    NO_COMMANDS,
  ]

  const update = (
    model: SidebarModel,
    message: SidebarNavMessage,
  ): readonly [SidebarModel, SidebarCommands] => {
    if (message._tag === "SidebarItemActivated") {
      options.onActivate?.(message.value)
    }
    return [sidebarUpdate(model, message), NO_COMMANDS]
  }

  const view = (model: SidebarModel) =>
    sidebarView<SidebarNavMessage>({
      model,
      groups: navGroups(),
      toMessage: message => message,
      side: "left",
      label: "Khala Code navigation",
    })

  const program = Runtime.makeProgram<SidebarModel, SidebarNavMessage>({
    Model: SidebarModelSchema,
    init,
    update,
    view,
    container,
  })

  Runtime.run(program)
}
