import { Schema } from "effect"
import { Runtime } from "foldkit"
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
// Model, interactions flow through SidebarPrimitiveMessage + sidebarUpdate, and it
// renders through sidebarView. No manual DOM, no class-name shim.

export type SidebarNavMessage = SidebarPrimitiveMessage

type SidebarModel = SidebarPrimitiveModel

// This sidebar never issues commands; init/update return an empty command list.
const NO_COMMANDS: readonly never[] = []

// Foldkit requires a Schema codec for the Model.
const SidebarPrimitiveItemSchema = Schema.Struct({
  value: Schema.String,
})

const SidebarModelSchema = Schema.Struct({
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

const navItems = (): ReadonlyArray<{ readonly value: string }> =>
  navGroups().flatMap(group => group.items.map(item => ({ value: item.value })))

export type SidebarMountOptions = Readonly<{
  readonly selectedValue?: string | null
  readonly onActivate?: (value: string) => void
}>

export const mountKhalaCodeSidebar = (
  container: HTMLElement,
  options: SidebarMountOptions = {},
): void => {
  const init = (): readonly [SidebarModel, readonly never[]] => [
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
  ): readonly [SidebarModel, readonly never[]] => {
    if (message._tag === "SidebarItemActivated") {
      options.onActivate?.(message.value)
    }
    return [sidebarUpdate(model, message), NO_COMMANDS]
  }

  const view = (model: SidebarModel) => ({
    title: "Khala Code",
    body: sidebarView<SidebarNavMessage>({
      model,
      groups: navGroups(),
      toMessage: message => message,
      side: "left",
      label: "Khala Code navigation",
    }),
  })

  const program = Runtime.makeProgram({
    Model: SidebarModelSchema,
    init,
    update,
    view,
    container,
  })

  Runtime.run(program)
}
