import { Effect, Schema, SubscriptionRef } from "effect"
export * from "./khala-ui-parity.js"
export * from "./khala-ui-effects-gallery.js"
import {
  Accordion,
  Alert,
  AnnouncementBadge,
  Avatar,
  AvatarGroup,
  Badge,
  Button,
  Card,
  Checkbox,
  Chip,
  CodeBlock,
  Combobox,
  CommandPalette,
  Composer,
  ComponentValueBinding,
  ContextMenu,
  CopyButton,
  CtaSection,
  DiffView,
  Divider,
  DropdownMenu,
  EmptyMessage,
  emptyMessageIconSizes,
  emptyMessageIconTones,
  FieldRow,
  Footer,
  Glow,
  GraphFigure,
  Hero,
  Host,
  Icon,
  IconButton,
  Image,
  IntentRef,
  LogoRow,
  Meter,
  MockupFrame,
  NavBar,
  Pager,
  SwipeableListItem,
  BackgroundGradient,
  Wallpaper,
  Spotlight,
  Frame,
  BlurredPopup,
  NumberField,
  Popover,
  PricingColumn,
  PricingTable,
  RadioGroup,
  RecoveryOverlay,
  Section,
  SegmentedControl,
  Select,
  Slider,
  Spinner,
  LoadingDots,
  ShimmerText,
  StatTile,
  StatsBand,
  StatusBanner,
  Table,
  Timeline,
  Toast,
  ToastRegion,
  Toggle,
  Toolbar,
  Tooltip,
  Transcript,
  IntentSchema,
  JsonPayloadSchema,
  Link,
  List,
  Markdown,
  Modal,
  NavRail,
  SectionList,
  Sheet,
  Spacer,
  SplitPane,
  Stack,
  StaticPayload,
  Tabs,
  Text,
  TextField,
  Workbench,
  ViewSchema,
  componentTags,
  colorTokens,
  defaultTheme,
  hostKinds,
  iconNames,
  iconSizes,
  avatarVariants,
  tones,
  defineIntent,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  typeScaleTokens,
  type ComponentTag,
  type IntentHandlers,
  type IntentRegistry,
  type IntentReporter,
  type JsonPayload,
  type KeyedView,
  type Theme,
  type View,
  type ViewProgram,
  type ViewportInput
} from "@effect-native/core"
import {
  ThemeSchema,
  breakpointTokens,
  controlTokens,
  dimensionTokens,
  khalaTheme,
  radiusTokens,
  spacingTokens,
  toneTokens,
  toneVariantTokens
} from "@effect-native/tokens"
import {
  khalaUiArwesReference,
  khalaUiCapabilityMatrix,
  khalaUiGoldenFixtures,
  khalaUiMotifIds,
  khalaUiPerformanceBudgets,
  khalaUiProofCases,
  khalaUiRendererIds,
  khalaUiRestraintLimits
} from "./khala-ui-contract"

export * from "./khala-ui-contract"

export const packageName = "@effect-native/gallery" as const

const buttonVariants = ["primary", "secondary", "ghost"] as const
const stackDirections = ["row", "column"] as const
const imageFits = ["contain", "cover", "fill"] as const
const sheetEdges = ["bottom", "side"] as const
const dimensionOptions = ["sm", "md", "lg"] as const
const textWeights = ["regular", "medium", "semibold", "bold"] as const
const spacingControlOptions = ["0", "1", "2", "3", "4", "6", "8", "12"] as const
const radiusControlOptions = ["none", "sm", "md", "lg", "xl", "full"] as const

const genericSvg = [
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 960 420'%3E",
  "%3Crect width='960' height='420' fill='%232563eb'/%3E",
  "%3Cpath d='M0 330 C210 220 380 390 590 250 S790 185 960 230 V420 H0 Z' fill='%23f8fafc'/%3E",
  "%3Ccircle cx='750' cy='112' r='104' fill='%2393c5fd' opacity='.6'/%3E",
  "%3Ctext x='64' y='126' font-family='Arial' font-size='52' font-weight='700' fill='white'%3EComponent story%3C/text%3E",
  "%3Ctext x='68' y='184' font-family='Arial' font-size='26' fill='white'%3EOne fixture, every renderer%3C/text%3E",
  "%3C/svg%3E"
].join("")

// "Dark" is the one canonical Khala Protoss-blue theme (see @effect-native/tokens
// `khalaTheme` and issue #25) — the gallery previews it here rather than hand-
// rolling a second, drifting dark palette.
const darkTheme: Theme = khalaTheme

export const galleryThemes = [
  { id: "default", label: "Default", theme: defaultTheme },
  { id: "dark", label: "Dark", theme: darkTheme }
] as const

export type GalleryThemeId = (typeof galleryThemes)[number]["id"]

export const galleryViewports = [
  { id: "phone", label: "Phone", viewport: { width: 390, height: 844 } },
  { id: "tablet", label: "Tablet", viewport: { width: 820, height: 1180 } },
  { id: "desktop", label: "Desktop", viewport: { width: 1280, height: 832 } }
] as const satisfies ReadonlyArray<{
  readonly id: string
  readonly label: string
  readonly viewport: ViewportInput
}>

export type GalleryViewportId = (typeof galleryViewports)[number]["id"]

export const StoryKindSchema = Schema.Literals(["generated", "hand-authored"] as const)
export type StoryKind = Schema.Schema.Type<typeof StoryKindSchema>

export const StoryControlKindSchema = Schema.Literals(["boolean", "enum", "text", "number", "token"] as const)
export type StoryControlKind = Schema.Schema.Type<typeof StoryControlKindSchema>

export const StoryPathSegmentSchema = Schema.Union([Schema.String, Schema.Number])
export type StoryPathSegment = Schema.Schema.Type<typeof StoryPathSegmentSchema>

export const StoryControlSchema = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.NonEmptyString,
  kind: StoryControlKindSchema,
  path: Schema.Array(StoryPathSegmentSchema),
  value: JsonPayloadSchema,
  options: Schema.Array(Schema.String).pipe(Schema.optionalKey)
})
export type StoryControl = Schema.Schema.Type<typeof StoryControlSchema>

export const StoryInteractionSchema = Schema.Struct({
  label: Schema.NonEmptyString,
  intent: IntentSchema,
  runtimeValue: JsonPayloadSchema.pipe(Schema.optionalKey)
})
export type StoryInteraction = Schema.Schema.Type<typeof StoryInteractionSchema>

export const StorySchema = Schema.Struct({
  version: Schema.Literal("effect-native/story/v0"),
  id: Schema.NonEmptyString,
  component: Schema.Literals(componentTags),
  title: Schema.NonEmptyString,
  description: Schema.String,
  kind: StoryKindSchema,
  view: ViewSchema,
  theme: ThemeSchema,
  viewport: Schema.Struct({
    width: Schema.Number,
    height: Schema.Number
  }),
  controls: Schema.Array(StoryControlSchema),
  interactions: Schema.Array(StoryInteractionSchema)
})
export type Story = Schema.Schema.Type<typeof StorySchema>

export const StoryGroupSchema = Schema.Struct({
  component: Schema.Literals(componentTags),
  title: Schema.NonEmptyString,
  stories: Schema.Array(StorySchema)
})
export type StoryGroup = Schema.Schema.Type<typeof StoryGroupSchema>

export const StorybookSchema = Schema.Struct({
  version: Schema.Literal("effect-native/storybook/v0"),
  title: Schema.NonEmptyString,
  groups: Schema.Array(StoryGroupSchema)
})
export type Storybook = Schema.Schema.Type<typeof StorybookSchema>

const keyed = <V extends View>(view: V): V & { readonly key: string } => view as V & { readonly key: string }

const story = (input: {
  readonly id: string
  readonly component: ComponentTag
  readonly title: string
  readonly description: string
  readonly kind?: StoryKind
  readonly view: View
  readonly controls?: ReadonlyArray<StoryControl>
  readonly interactions?: ReadonlyArray<StoryInteraction>
  readonly viewport?: ViewportInput
  readonly theme?: Theme
}): Story =>
  StorySchema.make({
    version: "effect-native/story/v0",
    id: input.id,
    component: input.component,
    title: input.title,
    description: input.description,
    view: input.view,
    kind: input.kind ?? "generated",
    controls: input.controls ?? [],
    interactions: input.interactions ?? [],
    viewport: input.viewport ?? galleryViewports[2].viewport,
    theme: input.theme ?? defaultTheme
  })

const enumControl = (
  id: string,
  label: string,
  path: ReadonlyArray<StoryPathSegment>,
  value: string,
  options: ReadonlyArray<string>
): StoryControl => StoryControlSchema.make({ id, label, kind: "enum", path, value, options })

const tokenControl = (
  id: string,
  label: string,
  path: ReadonlyArray<StoryPathSegment>,
  value: string,
  options: ReadonlyArray<string>
): StoryControl => StoryControlSchema.make({ id, label, kind: "token", path, value, options })

const booleanControl = (
  id: string,
  label: string,
  path: ReadonlyArray<StoryPathSegment>,
  value: boolean
): StoryControl => StoryControlSchema.make({ id, label, kind: "boolean", path, value })

const textControl = (id: string, label: string, path: ReadonlyArray<StoryPathSegment>, value: string): StoryControl =>
  StoryControlSchema.make({ id, label, kind: "text", path, value })

const numberControl = (id: string, label: string, path: ReadonlyArray<StoryPathSegment>, value: number): StoryControl =>
  StoryControlSchema.make({ id, label, kind: "number", path, value })

const Pressed = defineIntent("GalleryStory.Pressed", Schema.Struct({ id: Schema.String }))
const Changed = defineIntent("GalleryStory.Changed", Schema.String)
const Submitted = defineIntent("GalleryStory.Submitted", Schema.String)
const Dismissed = defineIntent("GalleryStory.Dismissed", Schema.Struct({ surface: Schema.String }))

export const storyIntentDefinitions = [Pressed, Changed, Submitted, Dismissed] as const

const cardCopy = (key: string, content = "Card content"): ReadonlyArray<View> => [
  Text({ key: `${key}-title`, content, variant: "label", color: "textPrimary" }),
  Text({
    key: `${key}-copy`,
    content: "A generic story fixture rendered from serializable data.",
    variant: "body",
    color: "textMuted",
    style: { marginTop: "1" }
  })
]

const listItems: ReadonlyArray<KeyedView> = [
  keyed(Text({ key: "item-one", content: "Queued", variant: "body" })),
  keyed(Text({ key: "item-two", content: "Running", variant: "body" })),
  keyed(Text({ key: "item-three", content: "Complete", variant: "body" }))
]

const sectionItems: ReadonlyArray<KeyedView> = [
  keyed(Text({ key: "section-item-a", content: "Primary item", variant: "body" })),
  keyed(Text({ key: "section-item-b", content: "Secondary item", variant: "body" }))
]

const componentStoryMap = {
  Stack: [
    story({
      id: "stack-column",
      component: "Stack",
      title: "Column stack",
      description: "Generated baseline for Stack direction, spacing, and padding.",
      view: Stack(
        {
          key: "stack-column",
          direction: "column",
          gap: "2",
          padding: "3",
          style: { backgroundColor: "surface", borderColor: "border", borderWidth: 1 }
        },
        [
          Text({ key: "stack-one", content: "First row", variant: "body" }),
          Text({ key: "stack-two", content: "Second row", variant: "body" })
        ]
      ),
      controls: [
        enumControl("stack-direction", "Direction", ["direction"], "column", stackDirections),
        tokenControl("stack-gap", "Gap", ["gap"], "2", spacingControlOptions),
        tokenControl("stack-padding", "Padding", ["padding"], "3", spacingControlOptions)
      ]
    }),
    story({
      id: "stack-responsive",
      component: "Stack",
      title: "Responsive stack",
      description: "Hand-authored composition proving breakpoint data stays in the story.",
      kind: "hand-authored",
      view: Stack(
        {
          key: "stack-responsive",
          direction: { base: "column", md: "row" },
          gap: { base: "2", md: "4" },
          padding: "3",
          style: {
            borderColor: "border",
            borderWidth: 1,
            variants: { breakpoint: { md: { backgroundColor: "surface" } } }
          }
        },
        [
          Card({ key: "responsive-a", padding: "3", radius: "md" }, cardCopy("responsive-a", "Mobile first")),
          Card({ key: "responsive-b", padding: "3", radius: "md" }, cardCopy("responsive-b", "Desktop row"))
        ]
      )
    })
  ],
  Text: [
    ...typeScaleTokens.map((variant) =>
      story({
        id: `text-${variant}`,
        component: "Text",
        title: `${variant} text`,
        description: "Generated text type-scale coverage.",
        view: Text({
          key: `text-${variant}`,
          content: `The ${variant} text style`,
          variant,
          color: "textPrimary"
        }),
        controls: [
          textControl("text-content", "Content", ["content"], `The ${variant} text style`),
          enumControl("text-variant", "Variant", ["variant"], variant, typeScaleTokens),
          tokenControl("text-color", "Color", ["color"], "textPrimary", colorTokens),
          enumControl("text-weight", "Weight", ["weight"], "regular", textWeights)
        ]
      })
    )
  ],
  Button: buttonVariants.map((variant) =>
    story({
      id: `button-${variant}`,
      component: "Button",
      title: `${variant} button`,
      description: "Generated button variant and disabled-state coverage.",
      view: Button({
        key: `button-${variant}`,
        label: `${variant} action`,
        variant,
        onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ id: variant })),
        style: { borderRadius: "md", padding: "3" }
      }),
      controls: [
        textControl("button-label", "Label", ["label"], `${variant} action`),
        enumControl("button-variant", "Variant", ["variant"], variant, buttonVariants),
        booleanControl("button-disabled", "Disabled", ["disabled"], false)
      ],
      interactions: [
        {
          label: "Press",
          intent: { name: "GalleryStory.Pressed", payload: { id: variant } }
        }
      ]
    })
  ),
  Image: imageFits.map((fit) =>
    story({
      id: `image-${fit}`,
      component: "Image",
      title: `${fit} image`,
      description: "Generated image fit coverage using a public-safe data URI.",
      view: Image({
        key: `image-${fit}`,
        source: genericSvg,
        alt: "Generic component story image",
        width: "lg",
        height: 180,
        fit,
        style: { borderRadius: "lg" }
      }),
      controls: [
        enumControl("image-fit", "Fit", ["fit"], fit, imageFits),
        tokenControl("image-radius", "Radius", ["style", "borderRadius"], "lg", radiusControlOptions),
        numberControl("image-height", "Height", ["height"], 180)
      ]
    })
  ),
  TextField: [
    story({
      id: "textfield-basic",
      component: "TextField",
      title: "Text field",
      description: "Generated text field value, label, placeholder, focus, and multiline coverage.",
      view: TextField({
        key: "textfield-basic",
        label: "Name",
        value: "Ada",
        placeholder: "Enter text",
        onChange: IntentRef("GalleryStory.Changed", ComponentValueBinding()),
        onSubmit: IntentRef("GalleryStory.Submitted", ComponentValueBinding()),
        style: { borderColor: "border", borderWidth: 1, borderRadius: "md", padding: "3" }
      }),
      controls: [
        textControl("textfield-label", "Label", ["label"], "Name"),
        textControl("textfield-value", "Value", ["value"], "Ada"),
        textControl("textfield-placeholder", "Placeholder", ["placeholder"], "Enter text"),
        booleanControl("textfield-multiline", "Multiline", ["multiline"], false),
        booleanControl("textfield-focused", "Focused", ["focused"], false)
      ],
      interactions: [
        { label: "Type", intent: { name: "GalleryStory.Changed", payload: "Typed value" } },
        { label: "Submit", intent: { name: "GalleryStory.Submitted", payload: "Typed value" } }
      ]
    }),
    story({
      id: "textfield-secure",
      component: "TextField",
      title: "Secure field",
      description: "Generated secure-field state coverage.",
      view: TextField({
        key: "textfield-secure",
        label: "Secret",
        value: "secret",
        secure: true,
        onChange: IntentRef("GalleryStory.Changed", ComponentValueBinding()),
        style: { borderColor: "border", borderWidth: 1, borderRadius: "md", padding: "3" }
      }),
      controls: [
        textControl("textfield-secure-value", "Value", ["value"], "secret"),
        booleanControl("textfield-secure-focused", "Focused", ["focused"], false)
      ]
    }),
    story({
      id: "textfield-matrix-variant",
      component: "TextField",
      title: "TextField (matrix variant + size + invalid, harmonization #79)",
      description:
        "Opt-in `variant`/`size`/`gutterSize` matrix box chrome and `invalid` danger cue; omitting `variant`/`size` keeps the original renderer-drawn-chromeless look (style-driven boxes still work unchanged).",
      view: TextField({
        key: "textfield-matrix-variant",
        label: "Email",
        value: "",
        placeholder: "you@example.com",
        variant: "outline",
        size: "md",
        invalid: true,
        onChange: IntentRef("GalleryStory.Changed", ComponentValueBinding())
      }),
      controls: [
        enumControl("textfield-matrix-variant-select", "Variant", ["variant"], "outline", ["outline", "soft"]),
        enumControl("textfield-matrix-size", "Size", ["size"], "md", controlTokens),
        booleanControl("textfield-matrix-invalid", "Invalid", ["invalid"], true)
      ]
    }),
    story({
      id: "textfield-multiline-autoresize",
      component: "TextField",
      title: "TextField (multiline autoResize, Textarea parity)",
      description:
        "Plain multiline mode that grows its height to fit content (DOM); React Native already grows a multiline field with no fixed height, so `autoResize` is a declared no-op there.",
      view: TextField({
        key: "textfield-multiline-autoresize",
        label: "Notes",
        value: "A note that can grow across multiple lines as you type more content.",
        multiline: true,
        autoResize: true,
        variant: "outline",
        onChange: IntentRef("GalleryStory.Changed", ComponentValueBinding())
      }),
      controls: [booleanControl("textfield-autoresize", "Auto-resize", ["autoResize"], true)]
    })
  ],
  List: [
    story({
      id: "list-basic",
      component: "List",
      title: "List",
      description: "Generated list and virtualization coverage.",
      view: List(
        {
          key: "list-basic",
          style: { borderColor: "border", borderWidth: 1, padding: "2", borderRadius: "md" }
        },
        listItems
      ),
      controls: [
        booleanControl("list-virtualize", "Virtualize", ["virtualize"], false),
        numberControl("list-estimated-size", "Estimated item size", ["estimatedItemSize"], 48)
      ]
    }),
    story({
      id: "list-virtualized",
      component: "List",
      title: "Virtualized list",
      description: "Hand-authored virtualized list fixture.",
      kind: "hand-authored",
      view: List(
        {
          key: "list-virtualized",
          virtualize: true,
          estimatedItemSize: 48,
          style: { borderColor: "border", borderWidth: 1, padding: "2", borderRadius: "md" }
        },
        [...listItems, keyed(Text({ key: "item-four", content: "Archived", variant: "body" }))]
      )
    })
  ],
  SectionList: [
    story({
      id: "sectionlist-basic",
      component: "SectionList",
      title: "Section list",
      description: "Generated section list and sticky header coverage.",
      view: SectionList(
        {
          key: "sectionlist-basic",
          stickyHeaders: true,
          style: { borderColor: "border", borderWidth: 1, padding: "2", borderRadius: "md" }
        },
        [
          {
            key: "first-section",
            header: Text({ key: "first-header", content: "First section", variant: "label" }),
            items: sectionItems
          }
        ]
      ),
      controls: [
        booleanControl("sectionlist-sticky", "Sticky headers", ["stickyHeaders"], true),
        booleanControl("sectionlist-virtualize", "Virtualize", ["virtualize"], false),
        numberControl("sectionlist-estimated-size", "Estimated item size", ["estimatedItemSize"], 52)
      ]
    })
  ],
  Card: [
    story({
      id: "card-basic",
      component: "Card",
      title: "Card",
      description: "Generated card padding, radius, and surface coverage.",
      view: Card(
        {
          key: "card-basic",
          padding: "4",
          radius: "lg",
          style: { backgroundColor: "surface", borderColor: "border", borderWidth: 1 }
        },
        cardCopy("card-basic")
      ),
      controls: [
        tokenControl("card-padding", "Padding", ["padding"], "4", spacingControlOptions),
        tokenControl("card-radius", "Radius", ["radius"], "lg", radiusControlOptions)
      ]
    })
  ],
  Spacer: [
    story({
      id: "spacer-size",
      component: "Spacer",
      title: "Fixed spacer",
      description: "Generated fixed spacer coverage.",
      view: Stack({ key: "spacer-size-wrap", direction: "row", align: "center", gap: "1" }, [
        Text({ key: "spacer-before", content: "Before", variant: "body" }),
        Spacer({ key: "spacer-size", size: "8" }),
        Text({ key: "spacer-after", content: "After", variant: "body" })
      ]),
      controls: [tokenControl("spacer-size", "Size", ["children", 1, "size"], "8", spacingControlOptions)]
    }),
    story({
      id: "spacer-flex",
      component: "Spacer",
      title: "Flex spacer",
      description: "Generated flex spacer coverage.",
      view: Stack(
        {
          key: "spacer-flex-wrap",
          direction: "row",
          align: "center",
          style: { width: "full" }
        },
        [
          Text({ key: "spacer-flex-before", content: "Start", variant: "body" }),
          Spacer({ key: "spacer-flex", flex: true }),
          Text({ key: "spacer-flex-after", content: "End", variant: "body" })
        ]
      )
    })
  ],
  Link: [
    story({
      id: "link-path",
      component: "Link",
      title: "Path link",
      description: "Generated typed navigation destination coverage.",
      view: Link(
        {
          key: "link-path",
          destination: { kind: "path", path: "/docs" },
          style: { color: "accent", padding: "2", borderRadius: "md" }
        },
        [Text({ key: "link-path-label", content: "Read the docs", variant: "body" })]
      ),
      controls: [textControl("link-label", "Label", ["children", 0, "content"], "Read the docs")]
    }),
    story({
      id: "link-anchor",
      component: "Link",
      title: "Anchor link",
      description: "Generated anchor destination coverage.",
      view: Link(
        {
          key: "link-anchor",
          destination: { kind: "anchor", id: "component-preview" },
          style: { color: "accent", padding: "2" }
        },
        [Text({ key: "link-anchor-label", content: "Jump to preview", variant: "body" })]
      )
    })
  ],
  Modal: [
    story({
      id: "modal-open",
      component: "Modal",
      title: "Open modal",
      description: "Generated modal size, open, and dismissable-state coverage.",
      view: Modal(
        {
          key: "modal-open",
          title: "Confirm operation",
          open: true,
          dismissable: true,
          size: "md",
          onDismiss: IntentRef("GalleryStory.Dismissed", StaticPayload({ surface: "modal" }))
        },
        [Text({ key: "modal-open-copy", content: "Modal body content.", variant: "body" })]
      ),
      controls: [
        booleanControl("modal-open", "Open", ["open"], true),
        booleanControl("modal-dismissable", "Dismissable", ["dismissable"], true),
        enumControl("modal-size", "Size", ["size"], "md", dimensionOptions)
      ],
      interactions: [{ label: "Dismiss", intent: { name: "GalleryStory.Dismissed", payload: { surface: "modal" } } }]
    })
  ],
  Sheet: sheetEdges.map((edge) =>
    story({
      id: `sheet-${edge}`,
      component: "Sheet",
      title: `${edge} sheet`,
      description: "Generated sheet edge, detent, and open-state coverage.",
      view: Sheet(
        {
          key: `sheet-${edge}`,
          open: true,
          dismissable: true,
          edge,
          detents: ["sm", "md"],
          onDismiss: IntentRef("GalleryStory.Dismissed", StaticPayload({ surface: "sheet" }))
        },
        [Text({ key: `sheet-${edge}-copy`, content: "Sheet body content.", variant: "body" })]
      ),
      controls: [
        booleanControl("sheet-open", "Open", ["open"], true),
        enumControl("sheet-edge", "Edge", ["edge"], edge, sheetEdges)
      ],
      interactions: [{ label: "Dismiss", intent: { name: "GalleryStory.Dismissed", payload: { surface: "sheet" } } }]
    })
  ),
  Host: hostKinds.map((kind) =>
    story({
      id: `host-${kind}`,
      component: "Host",
      title: `${kind} host`,
      description: "Foreign-host escape hatch: serializable kind + props, driver-owned widget.",
      view: Host({
        key: `host-${kind}`,
        kind,
        props: { placeholder: `${kind} host props` },
        onEvent: IntentRef("GalleryStory.HostEvent", ComponentValueBinding()),
        style: { backgroundColor: "surface", borderColor: "border", borderWidth: 1 }
      })
    })
  ),
  Icon: [
    story({
      id: "icon-set",
      component: "Icon",
      title: "Icon set",
      description: "Closed icon-name set rendered from the per-renderer registry.",
      view: Stack(
        { key: "icon-set-wrap", direction: "row", gap: "2", align: "center" },
        iconNames.map((name) => Icon({ key: `icon-${name}`, name, size: "md", color: "textPrimary", label: name }))
      ),
      controls: [
        enumControl("icon-name", "Name", ["children", 0, "name"], iconNames[0], iconNames),
        enumControl("icon-size", "Size", ["children", 0, "size"], "md", iconSizes)
      ]
    })
  ],
  Divider: [
    story({
      id: "divider-horizontal",
      component: "Divider",
      title: "Divider",
      description: "Section separator with typed orientation.",
      view: Divider({ key: "divider-horizontal", orientation: "horizontal" }),
      controls: [
        enumControl("divider-orientation", "Orientation", ["orientation"], "horizontal", ["horizontal", "vertical"])
      ]
    })
  ],
  Badge: [
    story({
      id: "badge-tone",
      component: "Badge",
      title: "Badge",
      description: "Status/count badge with typed tone.",
      view: Badge({ key: "badge-tone", label: "Live", tone: "success" }),
      controls: [enumControl("badge-tone", "Tone", ["tone"], "success", tones)]
    }),
    story({
      id: "badge-matrix-variant",
      component: "Badge",
      title: "Badge (matrix variant + size, harmonization #79)",
      description:
        "Opt-in tone x variant x size matrix chrome via `variant`/`size`; omitting both keeps the original tone-colored-text-only look.",
      view: Badge({ key: "badge-matrix-variant", label: "Danger", tone: "danger", variant: "soft", size: "sm" }),
      controls: [
        enumControl("badge-matrix-tone", "Tone", ["tone"], "danger", tones),
        enumControl("badge-matrix-variant-select", "Variant", ["variant"], "soft", ["solid", "soft", "outline"]),
        enumControl("badge-matrix-size", "Size", ["size"], "sm", controlTokens)
      ]
    })
  ],
  Chip: [
    story({
      id: "chip-value",
      component: "Chip",
      title: "Chip",
      description: "Label + value pill for the fleet cockpit strip.",
      view: Chip({ key: "chip-value", label: "Slots", value: "3/8", tone: "info" }),
      controls: [enumControl("chip-tone", "Tone", ["tone"], "info", tones)]
    }),
    story({
      id: "chip-matrix-variant",
      component: "Chip",
      title: "Chip (matrix variant + size, harmonization #79)",
      description:
        "Opt-in tone x variant x size matrix chrome via `variant`/`size`; omitting both keeps the original look.",
      view: Chip({
        key: "chip-matrix-variant",
        label: "Workers",
        value: "3/8",
        tone: "info",
        variant: "outline",
        size: "sm"
      }),
      controls: [
        enumControl("chip-matrix-tone", "Tone", ["tone"], "info", tones),
        enumControl("chip-matrix-variant-select", "Variant", ["variant"], "outline", ["solid", "soft", "outline"]),
        enumControl("chip-matrix-size", "Size", ["size"], "sm", controlTokens)
      ]
    })
  ],
  Meter: [
    story({
      id: "meter-determinate",
      component: "Meter",
      title: "Meter",
      description: "Determinate capacity/budget readout.",
      view: Meter({ key: "meter-determinate", value: 0.5, label: "Capacity", tone: "info" }),
      controls: [enumControl("meter-tone", "Tone", ["tone"], "info", tones)]
    })
  ],
  StatTile: [
    story({
      id: "stat-tile",
      component: "StatTile",
      title: "StatTile",
      description: "Label + strong value summary cell.",
      view: StatTile({ key: "stat-tile", label: "Workers", value: "12", tone: "neutral" }),
      controls: [enumControl("stat-tone", "Tone", ["tone"], "neutral", tones)]
    })
  ],
  Table: [
    story({
      id: "table-basic",
      component: "Table",
      title: "Table",
      description: "Non-virtualized table with typed columns and keyed rows.",
      view: Table({
        key: "table-basic",
        columns: [
          { id: "name", header: "Name", align: "start" },
          { id: "status", header: "Status", align: "end" }
        ],
        rows: [
          {
            id: "row-1",
            cells: [
              Text({ key: "c1-name", content: "Orrery", variant: "body" }),
              Badge({ key: "c1-status", label: "ok", tone: "success" })
            ]
          },
          {
            id: "row-2",
            cells: [
              Text({ key: "c2-name", content: "Whitefang", variant: "body" }),
              Badge({ key: "c2-status", label: "offline", tone: "danger" })
            ]
          }
        ]
      })
    })
  ],
  SplitPane: [
    story({
      id: "split-pane-basic",
      component: "SplitPane",
      title: "SplitPane",
      description: "Resizable split layout; divider drag reports a typed { paneId, size } intent.",
      view: SplitPane({
        key: "split-pane-basic",
        orientation: "row",
        panes: [
          {
            id: "sidebar",
            size: 240,
            min: 160,
            max: 360,
            content: Text({ key: "sp-side", content: "Sidebar", variant: "body" })
          },
          { id: "main", content: Text({ key: "sp-main", content: "Main", variant: "body" }) }
        ],
        onResize: IntentRef("GalleryStory.Pressed", StaticPayload({ id: "split-pane-basic" })),
        onCollapseToggle: IntentRef("GalleryStory.Pressed", StaticPayload({ id: "split-pane-collapse" }))
      }),
      controls: [enumControl("split-orientation", "Orientation", ["orientation"], "row", stackDirections)]
    })
  ],
  NavRail: [
    story({
      id: "nav-rail-basic",
      component: "NavRail",
      title: "NavRail",
      description: "Navigation rail with sections and selectable items; selection is a typed intent.",
      view: NavRail({
        key: "nav-rail-basic",
        activeId: "chat",
        onSelect: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        sections: [
          {
            id: "workbench",
            label: "Workbench",
            items: [
              { id: "chat", label: "Chat", icon: "Circle" },
              { id: "editor", label: "Editor", icon: "Play" },
              { id: "terminal", label: "Terminal", icon: "Stop", disabled: true }
            ]
          }
        ]
      })
    })
  ],
  Workbench: [
    story({
      id: "workbench-basic",
      component: "Workbench",
      title: "Workbench",
      description: "Names and swaps the active pane as typed state — no mount/unmount closures.",
      view: Workbench({
        key: "workbench-basic",
        activePaneId: "chat",
        panes: [
          { id: "chat", content: Text({ key: "wb-chat", content: "Chat pane", variant: "body" }) },
          { id: "editor", content: Text({ key: "wb-editor", content: "Editor pane", variant: "body" }) }
        ]
      }),
      controls: [booleanControl("workbench-keep-mounted", "Keep mounted", ["keepMounted"], false)]
    })
  ],
  Popover: [
    story({
      id: "popover-basic",
      component: "Popover",
      title: "Popover",
      description: "Anchored floating surface; presence is typed state, dismiss is a typed intent.",
      view: Popover(
        {
          key: "popover-basic",
          open: true,
          placement: { side: "bottom", align: "start" },
          anchorKey: "popover-anchor",
          dismissable: true,
          onDismiss: IntentRef("GalleryStory.Dismissed", StaticPayload({ surface: "popover" }))
        },
        [Text({ key: "popover-copy", content: "Thread token details", variant: "body" })]
      ),
      controls: [booleanControl("popover-open", "Open", ["open"], true)]
    })
  ],
  DropdownMenu: [
    story({
      id: "dropdown-menu-basic",
      component: "DropdownMenu",
      title: "DropdownMenu",
      description: "Keyboard-navigable menu from a typed item model; per-item onSelect.",
      view: DropdownMenu({
        key: "dropdown-menu-basic",
        open: true,
        placement: { side: "bottom", align: "start" },
        onSelect: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        onDismiss: IntentRef("GalleryStory.Dismissed", StaticPayload({ surface: "dropdown-menu" })),
        items: [
          { id: "rename", label: "Rename", icon: "Reload", keybinding: "F2" },
          { id: "delete", label: "Delete", icon: "X", danger: true },
          { id: "archived", label: "Archived", disabled: true }
        ]
      }),
      controls: [booleanControl("dropdown-open", "Open", ["open"], true)]
    })
  ],
  ContextMenu: [
    story({
      id: "context-menu-basic",
      component: "ContextMenu",
      title: "ContextMenu",
      description: "Pointer-anchored menu at a typed position; same item model as DropdownMenu.",
      view: ContextMenu({
        key: "context-menu-basic",
        open: true,
        x: 120,
        y: 80,
        onSelect: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        onDismiss: IntentRef("GalleryStory.Dismissed", StaticPayload({ surface: "context-menu" })),
        items: [
          { id: "open", label: "Open" },
          { id: "copy-path", label: "Copy path", keybinding: "⌘C" }
        ]
      })
    })
  ],
  Tooltip: [
    story({
      id: "tooltip-basic",
      component: "Tooltip",
      title: "Tooltip",
      description: "Non-interactive hover/focus label wrapping one target; aria-describedby.",
      view: Tooltip(
        {
          key: "tooltip-basic",
          content: "Run the current cell",
          placement: { side: "top", align: "center" },
          delayMillis: 200
        },
        [Icon({ key: "tooltip-target", name: "Play", size: "md", label: "Run" })]
      )
    })
  ],
  Combobox: [
    story({
      id: "combobox-basic",
      component: "Combobox",
      title: "Combobox",
      description: "Typeahead with app-supplied results, roving aria-activedescendant, and typed intents.",
      view: Combobox({
        key: "combobox-basic",
        query: "op",
        placeholder: "Search commands…",
        highlightedId: "open-file",
        onQueryChange: IntentRef("GalleryStory.Changed", ComponentValueBinding()),
        onHighlight: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        onSelect: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        options: [
          {
            id: "open-file",
            label: "Open file",
            subtitle: "Jump to a workspace file",
            icon: "Play",
            group: "Navigation",
            keybinding: "⌘P"
          },
          { id: "open-recent", label: "Open recent", group: "Navigation" },
          {
            id: "reload",
            label: "Reload window",
            group: "Session",
            icon: "Reload",
            disabled: true,
            disabledReason: "Unavailable while a turn is streaming"
          }
        ]
      })
    })
  ],
  CommandPalette: [
    story({
      id: "command-palette-basic",
      component: "CommandPalette",
      title: "CommandPalette",
      description: "Modal-overlay composition of a Combobox on the presence primitive.",
      view: CommandPalette({
        key: "command-palette-basic",
        open: true,
        title: "Command palette",
        onDismiss: IntentRef("GalleryStory.Dismissed", StaticPayload({ surface: "command-palette" })),
        combobox: Combobox({
          key: "command-palette-combobox",
          query: "",
          placeholder: "Type a command…",
          highlightedId: "composer",
          onQueryChange: IntentRef("GalleryStory.Changed", ComponentValueBinding()),
          onHighlight: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
          onSelect: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
          options: [
            { id: "composer", label: "Focus composer", group: "Composer", keybinding: "⌘I" },
            { id: "files", label: "Go to file", group: "Files", keybinding: "⌘P" }
          ]
        })
      }),
      controls: [booleanControl("command-palette-open", "Open", ["open"], true)]
    })
  ],
  Tabs: [
    story({
      id: "tabs-basic",
      component: "Tabs",
      title: "Tabs",
      description: "WAI-ARIA tablist with roving tabindex; panel association by id, typed onSelect.",
      view: Tabs({
        key: "tabs-basic",
        selectedId: "chat",
        orientation: "horizontal",
        onSelect: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        tabs: [
          { id: "chat", label: "Chat", icon: "Circle" },
          { id: "editor", label: "Editor", icon: "Play", badge: "2" },
          { id: "terminal", label: "Terminal", disabled: true }
        ],
        panels: [
          { id: "chat", content: Text({ key: "tab-chat", content: "Chat panel", variant: "body" }) },
          { id: "editor", content: Text({ key: "tab-editor", content: "Editor panel", variant: "body" }) },
          { id: "terminal", content: Text({ key: "tab-terminal", content: "Terminal panel", variant: "body" }) }
        ]
      }),
      controls: [booleanControl("tabs-keep-mounted", "Keep mounted", ["keepMounted"], false)]
    })
  ],
  Composer: [
    story({
      id: "composer-basic",
      component: "Composer",
      title: "Composer",
      description: "Contenteditable chat input over a typed document with slash/mention autocomplete.",
      view: Composer({
        key: "composer-basic",
        mode: "normal",
        placeholder: "Message Khala…",
        doc: [
          { kind: "text", text: "Ship the " },
          { kind: "mention", id: "orrery", label: "@Orrery" },
          { kind: "text", text: " changes" }
        ],
        attachments: [{ id: "att-1", name: "diff.patch", mimeType: "text/x-patch", size: 2048 }],
        onChange: IntentRef("GalleryStory.Changed", ComponentValueBinding()),
        onSubmit: IntentRef("GalleryStory.Submitted", ComponentValueBinding()),
        onKeyCommand: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        onAttachmentDrop: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        autocomplete: {
          trigger: "slash",
          query: "run",
          combobox: Combobox({
            key: "composer-autocomplete",
            query: "run",
            highlightedId: "run-cell",
            onSelect: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
            options: [
              { id: "run-cell", label: "/run", subtitle: "Run the current cell", group: "Commands" },
              { id: "reset", label: "/reset", group: "Commands" }
            ]
          })
        }
      })
    })
  ],
  Toggle: [
    story({
      id: "toggle-basic",
      component: "Toggle",
      title: "Toggle",
      description: "Boolean setting; typed value + onChange, disabled/invalid state.",
      view: Toggle({
        key: "toggle-basic",
        value: true,
        label: "Auto-approve safe edits",
        onChange: IntentRef("GalleryStory.Pressed", ComponentValueBinding())
      }),
      controls: [booleanControl("toggle-value", "Value", ["value"], true)]
    })
  ],
  Select: [
    story({
      id: "select-basic",
      component: "Select",
      title: "Select",
      description: "Single-choice from a typed option list.",
      view: Select({
        key: "select-basic",
        value: "claude",
        label: "Model",
        onChange: IntentRef("GalleryStory.Changed", ComponentValueBinding()),
        options: [
          { value: "claude", label: "Claude" },
          { value: "codex", label: "Codex" },
          { value: "local", label: "Local", disabled: true }
        ]
      })
    }),
    story({
      id: "select-matrix-variant",
      component: "Select",
      title: "Select trigger (matrix variant + size + pill + dropdownIcon, harmonization #79)",
      description:
        "Opt-in tone-neutral trigger chrome via `variant`/`size`/`pill`/`dropdownIcon`; omitting `variant`/`size` keeps the pre-#79 platform-default `<select>` look.",
      view: Select({
        key: "select-matrix-variant",
        value: "claude",
        label: "Model",
        variant: "soft",
        size: "sm",
        pill: true,
        dropdownIcon: "ChevronDown",
        onChange: IntentRef("GalleryStory.Changed", ComponentValueBinding()),
        options: [
          { value: "claude", label: "Claude" },
          { value: "codex", label: "Codex" },
          { value: "local", label: "Local", disabled: true }
        ]
      }),
      controls: [
        enumControl("select-matrix-variant-select", "Variant", ["variant"], "soft", ["soft", "outline", "ghost"]),
        enumControl("select-matrix-size", "Size", ["size"], "sm", controlTokens),
        booleanControl("select-matrix-pill", "Pill", ["pill"], true)
      ]
    }),
    story({
      id: "select-multiple",
      component: "Select",
      title: "Select (multi-select, harmonization #79)",
      description: "Additive multi-select: `multiple` + `values`, onChange fires the next selected-values array.",
      view: Select({
        key: "select-multiple",
        value: "claude",
        multiple: true,
        values: ["claude", "codex"],
        label: "Models",
        variant: "outline",
        onChange: IntentRef("GalleryStory.Changed", ComponentValueBinding()),
        options: [
          { value: "claude", label: "Claude" },
          { value: "codex", label: "Codex" },
          { value: "local", label: "Local" }
        ]
      })
    })
  ],
  Checkbox: [
    story({
      id: "checkbox-basic",
      component: "Checkbox",
      title: "Checkbox",
      description: "Multi-select boolean; typed checked + onChange.",
      view: Checkbox({
        key: "checkbox-basic",
        checked: true,
        label: "Stream tokens",
        onChange: IntentRef("GalleryStory.Pressed", ComponentValueBinding())
      }),
      controls: [booleanControl("checkbox-checked", "Checked", ["checked"], true)]
    })
  ],
  RadioGroup: [
    story({
      id: "radio-group-basic",
      component: "RadioGroup",
      title: "RadioGroup",
      description: "Exclusive choice; typed value + onChange.",
      view: RadioGroup({
        key: "radio-group-basic",
        name: "harness-mode",
        value: "review",
        label: "Harness mode",
        onChange: IntentRef("GalleryStory.Changed", ComponentValueBinding()),
        options: [
          { value: "review", label: "Review" },
          { value: "auto", label: "Autonomous" }
        ]
      })
    })
  ],
  SegmentedControl: [
    story({
      id: "segmented-control-basic",
      component: "SegmentedControl",
      title: "SegmentedControl",
      description: "Single-choice control with an animated selection thumb; lattice size, gutterSize, and pill.",
      view: SegmentedControl({
        key: "segmented-control-basic",
        value: "review",
        size: "md",
        onChange: IntentRef("GalleryStory.Changed", ComponentValueBinding()),
        options: [
          { id: "review", label: "Review", icon: "Circle" },
          { id: "auto", label: "Autonomous" },
          { id: "shadow", label: "Shadow", disabled: true }
        ]
      }),
      controls: [
        enumControl("segmented-control-size", "Size", ["size"], "md", controlTokens),
        booleanControl("segmented-control-pill", "Pill", ["pill"], false)
      ]
    })
  ],
  Slider: [
    story({
      id: "slider-basic",
      component: "Slider",
      title: "Slider",
      description: "Bounded numeric range; typed value + onChange.",
      view: Slider({
        key: "slider-basic",
        value: 40,
        min: 0,
        max: 100,
        step: 5,
        label: "Temperature",
        onChange: IntentRef("GalleryStory.Pressed", ComponentValueBinding())
      }),
      controls: [numberControl("slider-value", "Value", ["value"], 40)]
    })
  ],
  NumberField: [
    story({
      id: "number-field-basic",
      component: "NumberField",
      title: "NumberField",
      description: "Bounded numeric input with step/min/max.",
      view: NumberField({
        key: "number-field-basic",
        value: 8,
        min: 1,
        max: 32,
        step: 1,
        label: "Max parallel workers",
        onChange: IntentRef("GalleryStory.Pressed", ComponentValueBinding())
      }),
      controls: [numberControl("number-field-value", "Value", ["value"], 8)]
    })
  ],
  FieldRow: [
    story({
      id: "field-row-basic",
      component: "FieldRow",
      title: "FieldRow",
      description: "Label + control + description + error layout for settings panels.",
      view: FieldRow({
        key: "field-row-basic",
        label: "Auto-approve safe edits",
        description: "Apply low-risk edits without a manual review step.",
        controlKey: "field-row-toggle",
        control: Toggle({
          key: "field-row-toggle",
          value: false,
          onChange: IntentRef("GalleryStory.Pressed", ComponentValueBinding())
        })
      })
    })
  ],
  Toast: [
    story({
      id: "toast-basic",
      component: "Toast",
      title: "Toast",
      description: "A single transient notification with a11y live region and dismiss.",
      view: Toast({
        key: "toast-basic",
        notification: {
          id: "turn-failed",
          tone: "danger",
          title: "Turn failed",
          detail: "The model connection dropped.",
          actionLabel: "Retry",
          action: IntentRef("GalleryStory.Pressed", ComponentValueBinding())
        },
        onDismiss: IntentRef("GalleryStory.Dismissed", StaticPayload({ surface: "toast" }))
      })
    })
  ],
  ToastRegion: [
    story({
      id: "toast-region-basic",
      component: "ToastRegion",
      title: "ToastRegion",
      description: "A stacked, placement-aware notification region.",
      view: ToastRegion({
        key: "toast-region-basic",
        placement: "bottom-end",
        onDismiss: IntentRef("GalleryStory.Dismissed", ComponentValueBinding()),
        notifications: [
          { id: "saved", tone: "success", title: "Settings saved" },
          { id: "update", tone: "info", title: "Update available", detail: "Restart to apply." }
        ]
      })
    })
  ],
  StatusBanner: [
    story({
      id: "status-banner-basic",
      component: "StatusBanner",
      title: "StatusBanner",
      description: "A persistent inline banner with typed tone + retry/dismiss.",
      view: StatusBanner({
        key: "status-banner-basic",
        tone: "warn",
        message: "Boot RPC degraded: retrying capture stream.",
        onRetry: IntentRef("GalleryStory.Pressed", StaticPayload({ id: "retry" })),
        onDismiss: IntentRef("GalleryStory.Dismissed", StaticPayload({ surface: "status-banner" }))
      })
    })
  ],
  Alert: [
    story({
      id: "alert-basic",
      component: "Alert",
      title: "Alert (harmonization #79)",
      description:
        "Icon + title + body callout on the tone x variant matrix — a new component distinct from StatusBanner's persistent single-line banner role.",
      view: Alert({
        key: "alert-basic",
        tone: "warning",
        variant: "soft",
        title: "Review recommended",
        message: "This change touches a shared schema file. Confirm the migration before merging.",
        onDismiss: IntentRef("GalleryStory.Dismissed", StaticPayload({ surface: "alert" }))
      }),
      controls: [
        enumControl("alert-tone", "Tone", ["tone"], "warning", toneTokens),
        enumControl("alert-variant", "Variant", ["variant"], "soft", toneVariantTokens),
        textControl("alert-title", "Title", ["title"], "Review recommended"),
        textControl(
          "alert-message",
          "Message",
          ["message"],
          "This change touches a shared schema file. Confirm the migration before merging."
        )
      ]
    }),
    story({
      id: "alert-danger-no-title",
      component: "Alert",
      title: "Alert (danger, message-only)",
      description: "`title` is optional; `icon` defaults to a tone-appropriate glyph when omitted.",
      view: Alert({
        key: "alert-danger-no-title",
        tone: "danger",
        variant: "outline",
        message: "Failed to reach the workroom sidecar. Retrying automatically."
      })
    })
  ],
  RecoveryOverlay: [
    story({
      id: "recovery-overlay-basic",
      component: "RecoveryOverlay",
      title: "RecoveryOverlay",
      description: "A full-surface blocking overlay with typed recovery actions.",
      view: RecoveryOverlay({
        key: "recovery-overlay-basic",
        open: true,
        title: "Recovering session",
        status: "Reconnecting to the desktop bridge…",
        message: "Your work is safe. Choose how to proceed.",
        actions: [
          {
            id: "retry",
            label: "Retry now",
            variant: "primary",
            action: IntentRef("GalleryStory.Pressed", StaticPayload({ id: "retry" }))
          },
          {
            id: "restart",
            label: "Restart",
            variant: "secondary",
            action: IntentRef("GalleryStory.Pressed", StaticPayload({ id: "restart" }))
          }
        ]
      }),
      controls: [booleanControl("recovery-overlay-open", "Open", ["open"], true)]
    })
  ],
  Markdown: [
    story({
      id: "markdown-basic",
      component: "Markdown",
      title: "Markdown",
      description: "Renders a pre-parsed typed block+inline model — no parser, no raw HTML.",
      view: Markdown({
        key: "markdown-basic",
        blocks: [
          { kind: "heading", level: 2, children: [{ kind: "text", text: "Plan" }] },
          {
            kind: "paragraph",
            children: [
              { kind: "text", text: "Ship the " },
              { kind: "strong", children: [{ kind: "text", text: "diff" }] },
              { kind: "text", text: " with " },
              { kind: "code", text: "make test" }
            ]
          },
          {
            kind: "list",
            ordered: false,
            items: [
              [{ kind: "paragraph", children: [{ kind: "text", text: "Run the suite" }] }],
              [{ kind: "paragraph", children: [{ kind: "text", text: "Open a PR" }] }]
            ]
          }
        ]
      })
    })
  ],
  Transcript: [
    story({
      id: "transcript-basic",
      component: "Transcript",
      title: "Transcript",
      description: "A keyed, append-optimized log of role-styled messages with typed status.",
      view: Transcript({
        key: "transcript-basic",
        pinToEnd: true,
        onPinnedChange: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        messages: [
          {
            key: "m1",
            role: "user",
            body: [
              Markdown({
                key: "m1-md",
                blocks: [{ kind: "paragraph", children: [{ kind: "text", text: "Fix the failing test." }] }]
              })
            ]
          },
          {
            key: "m2",
            role: "assistant",
            status: "streaming",
            body: [
              Markdown({
                key: "m2-md",
                blocks: [{ kind: "paragraph", children: [{ kind: "text", text: "On it — running the suite now." }] }]
              })
            ]
          }
        ]
      })
    })
  ],
  CodeBlock: [
    story({
      id: "code-block-basic",
      component: "CodeBlock",
      title: "CodeBlock",
      description: "Renders pre-tokenized lines with blue-theme syntax colors and line numbers.",
      view: CodeBlock({
        key: "code-block-basic",
        language: "typescript",
        showLineNumbers: true,
        onCopy: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        lines: [
          {
            tokens: [
              { kind: "keyword", text: "const" },
              { kind: "plain", text: " x " },
              { kind: "operator", text: "=" },
              { kind: "plain", text: " " },
              { kind: "number", text: "1" }
            ]
          },
          {
            tokens: [
              { kind: "keyword", text: "function" },
              { kind: "plain", text: " " },
              { kind: "function", text: "run" },
              { kind: "operator", text: "()" }
            ]
          }
        ]
      })
    })
  ],
  DiffView: [
    story({
      id: "diff-view-basic",
      component: "DiffView",
      title: "DiffView",
      description: "A pre-parsed unified diff with add/remove theming and per-line review affordances.",
      view: DiffView({
        key: "diff-view-basic",
        language: "typescript",
        layout: "unified",
        onLineVerdict: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        onLineComment: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        onSourceControlAction: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        actions: [
          { id: "approve", label: "Approve" },
          { id: "stage", label: "Stage" }
        ],
        hunks: [
          {
            header: "@@ -1,2 +1,2 @@",
            rows: [
              {
                kind: "context",
                oldLine: 1,
                newLine: 1,
                tokens: [
                  { kind: "keyword", text: "const" },
                  { kind: "plain", text: " x" }
                ]
              },
              {
                kind: "remove",
                oldLine: 2,
                id: "r-2",
                verdict: "pending",
                tokens: [{ kind: "plain", text: "  return 1" }]
              },
              { kind: "add", newLine: 2, id: "r-3", tokens: [{ kind: "plain", text: "  return 2" }] }
            ]
          }
        ]
      })
    })
  ],
  GraphFigure: [
    story({
      id: "graph-figure-basic",
      component: "GraphFigure",
      title: "GraphFigure",
      description: "Arbiter graph over the canvas renderer with a DOM/SVG fallback from the same typed model.",
      view: GraphFigure({
        key: "graph-figure-basic",
        layout: "precomputed",
        width: 320,
        height: 200,
        camera: { x: 0, y: 0, zoom: 1 },
        onNodeSelect: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        onNodeHover: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        onCameraChange: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        nodes: [
          { id: "orrery", label: "Orrery", kind: "worker", status: "active", x: -80, y: 0 },
          { id: "whitefang", label: "Whitefang", kind: "validator", status: "success", x: 80, y: -40 },
          { id: "arbiter", label: "Arbiter", kind: "arbiter", status: "idle", x: 0, y: 40 }
        ],
        edges: [
          { id: "e1", from: "orrery", to: "arbiter", kind: "flow", status: "active" },
          { id: "e2", from: "arbiter", to: "whitefang", kind: "pairing", status: "success" }
        ]
      })
    })
  ],
  Timeline: [
    story({
      id: "timeline-basic",
      component: "Timeline",
      title: "Timeline",
      description: "A run timeline of typed events beside the graph.",
      view: Timeline({
        key: "timeline-basic",
        onEventSelect: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        events: [
          { id: "ev1", label: "Pairing opened", time: "12:00", status: "active", refs: ["orrery", "whitefang"] },
          { id: "ev2", label: "Validated", detail: "Whitefang accepted", time: "12:03", status: "success" }
        ]
      })
    })
  ],
  Section: [
    story({
      id: "section-contained",
      component: "Section",
      title: "Section",
      description: "Marketing layout band with contained width and vertical padding.",
      view: Section({ key: "section-contained", width: "contained", paddingY: "6", background: "surface" }, [
        Text({ key: "section-body", content: "Section content", variant: "body" })
      ])
    })
  ],
  Hero: [
    story({
      id: "hero-center",
      component: "Hero",
      title: "Hero",
      description: "Display-scale headline, subhead, CTA row, and optional media slot.",
      view: Hero({
        key: "hero-center",
        align: "center",
        headline: "Build with Effect Native",
        subhead: "One catalog. Every surface.",
        headlineTone: "gradient",
        actions: [
          Button({
            key: "hero-cta",
            label: "Get started",
            variant: "primary",
            onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
          })
        ],
        media: MockupFrame({ key: "hero-media", variant: "browser", tilt: "left" }, [
          Text({ key: "hero-media-label", content: "Product", variant: "body" })
        ])
      })
    })
  ],
  AnnouncementBadge: [
    story({
      id: "announcement-badge-basic",
      component: "AnnouncementBadge",
      title: "AnnouncementBadge",
      description: "Outlined pill above a hero with optional action intent.",
      view: AnnouncementBadge({
        key: "announcement-badge-basic",
        label: "Now open",
        actionLabel: "Read more",
        onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
      })
    })
  ],
  CtaSection: [
    story({
      id: "cta-section-basic",
      component: "CtaSection",
      title: "CtaSection",
      description: "Headline, supporting copy, and action row for mid-page conversion.",
      view: CtaSection({
        key: "cta-section-basic",
        headline: "Ready to ship?",
        body: "Start from a typed catalog and render everywhere.",
        tone: "info",
        actions: [
          Button({
            key: "cta-primary",
            label: "Open docs",
            variant: "primary",
            onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
          })
        ]
      })
    })
  ],
  Footer: [
    story({
      id: "footer-basic",
      component: "Footer",
      title: "Footer",
      description: "Brand slot, typed link columns, and legal row.",
      view: Footer({
        key: "footer-basic",
        brand: Text({ key: "footer-brand", content: "Effect Native", variant: "label" }),
        columns: [
          {
            id: "product",
            title: "Product",
            links: [
              Link({ key: "footer-docs", destination: { kind: "path", path: "/docs" } }, [
                Text({ key: "footer-docs-label", content: "Docs", variant: "body" })
              ])
            ]
          }
        ],
        legal: Text({ key: "footer-legal", content: "© OpenAgents", variant: "caption" })
      })
    })
  ],
  NavBar: [
    story({
      id: "navbar-basic",
      component: "NavBar",
      title: "NavBar",
      description: "Marketing top navigation with brand, links, actions, and collapse toggle.",
      view: NavBar({
        key: "navbar-basic",
        brand: Text({ key: "navbar-brand", content: "Effect Native", variant: "label" }),
        links: [
          {
            id: "docs",
            label: "Docs",
            onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
          },
          {
            id: "gallery",
            label: "Gallery",
            onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
          }
        ],
        sticky: true,
        collapsed: false,
        onToggleMenu: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 })),
        actions: [
          Button({
            key: "navbar-signin",
            label: "Sign in",
            variant: "ghost",
            onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
          })
        ]
      })
    })
  ],
  Accordion: [
    story({
      id: "accordion-faq",
      component: "Accordion",
      title: "Accordion",
      description: "FAQ-style disclosure list with typed expanded ids and toggle intent.",
      view: Accordion({
        key: "accordion-faq",
        mode: "single",
        expandedIds: ["q1"],
        onToggle: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        items: [
          {
            id: "q1",
            header: "What is Effect Native?",
            content: [
              Text({
                key: "faq-q1",
                content: "A closed, versioned UI catalog on Effect.",
                variant: "body"
              })
            ]
          },
          {
            id: "q2",
            header: "Where does it render?",
            content: [
              Text({
                key: "faq-q2",
                content: "Headless, DOM, and React Native today.",
                variant: "body"
              })
            ]
          }
        ]
      })
    })
  ],
  PricingColumn: [
    story({
      id: "pricing-column-basic",
      component: "PricingColumn",
      title: "PricingColumn",
      description: "Named plan, price, feature list, and CTA intent.",
      view: PricingColumn({
        key: "pricing-column-basic",
        name: "Starter",
        price: "$20",
        period: "mo",
        features: [
          { id: "credits", label: "Monthly credits", included: true },
          { id: "sso", label: "SSO", included: false }
        ],
        highlighted: true,
        ctaLabel: "Buy Starter",
        onCta: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
      })
    })
  ],
  PricingTable: [
    story({
      id: "pricing-table-basic",
      component: "PricingTable",
      title: "PricingTable",
      description: "Side-by-side pricing columns for plan comparison.",
      view: PricingTable({
        key: "pricing-table-basic",
        columns: [
          PricingColumn({
            key: "plan-starter",
            name: "Starter",
            price: "$20",
            period: "mo",
            features: [{ id: "f1", label: "Credits", included: true }],
            ctaLabel: "Buy",
            onCta: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
          }),
          PricingColumn({
            key: "plan-pro",
            name: "Pro",
            price: "$80",
            period: "mo",
            features: [
              { id: "f1", label: "Credits", included: true },
              { id: "f2", label: "Priority", included: true }
            ],
            highlighted: true,
            ctaLabel: "Upgrade",
            onCta: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
          })
        ]
      })
    })
  ],
  LogoRow: [
    story({
      id: "logo-row-basic",
      component: "LogoRow",
      title: "LogoRow",
      description: "Trusted-by logo strip with optional press intents.",
      view: LogoRow({
        key: "logo-row-basic",
        logos: [
          { id: "a", source: "https://example.com/a.svg", alt: "Alpha" },
          { id: "b", source: "https://example.com/b.svg", alt: "Beta" }
        ]
      })
    })
  ],
  StatsBand: [
    story({
      id: "stats-band-basic",
      component: "StatsBand",
      title: "StatsBand",
      description: "Metric band of bound values and labels.",
      view: StatsBand({
        key: "stats-band-basic",
        stats: [
          { id: "builders", label: "Builders", value: "12,400", tone: "info" },
          { id: "surfaces", label: "Surfaces", value: "3", tone: "success" }
        ]
      })
    })
  ],
  Glow: [
    story({
      id: "glow-basic",
      component: "Glow",
      title: "Glow",
      description: "Bounded radial accent glow behind a child slot.",
      view: Glow({ key: "glow-basic", intensity: "md" }, [
        Text({ key: "glow-child", content: "Highlighted", variant: "title" })
      ])
    })
  ],
  MockupFrame: [
    story({
      id: "mockup-frame-browser",
      component: "MockupFrame",
      title: "MockupFrame",
      description: "Browser/device frame with optional perspective tilt.",
      view: MockupFrame({ key: "mockup-frame-browser", variant: "browser", tilt: "left" }, [
        Text({ key: "mockup-child", content: "Product screenshot", variant: "body" })
      ])
    })
  ],
  Pager: [
    story({
      id: "pager-onboarding",
      component: "Pager",
      title: "Pager",
      description: "Linear onboarding stepper with progress dots and back/continue.",
      view: Pager({
        key: "pager-onboarding",
        activeStepId: "welcome",
        progress: "dots",
        canGoBack: false,
        canAdvance: true,
        onStepChange: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        onAdvance: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        onComplete: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        steps: [
          { id: "welcome", label: "Welcome" },
          { id: "repo", label: "Repo" },
          { id: "task", label: "Task" }
        ],
        panels: [
          {
            id: "welcome",
            content: Text({ key: "pager-welcome", content: "Welcome to Effect Native", variant: "body" })
          },
          {
            id: "repo",
            content: Text({ key: "pager-repo", content: "Choose a repository", variant: "body" })
          },
          {
            id: "task",
            content: Text({ key: "pager-task", content: "Describe your first task", variant: "body" })
          }
        ]
      })
    })
  ],
  SwipeableListItem: [
    story({
      id: "swipeable-list-item-basic",
      component: "SwipeableListItem",
      title: "SwipeableListItem",
      description: "List row with typed leading/trailing actions and onAction intent.",
      view: SwipeableListItem({
        key: "swipeable-list-item-basic",
        onAction: IntentRef("GalleryStory.Pressed", ComponentValueBinding()),
        leadingActions: [{ id: "pin", label: "Pin", icon: "Check" }],
        trailingActions: [
          { id: "quote", label: "Quote", tone: "info" },
          { id: "archive", label: "Archive", destructive: true, tone: "danger" }
        ],
        fullSwipeActionId: "archive",
        child: Text({ key: "swipe-label", content: "Thread title", variant: "body" })
      })
    })
  ],
  BackgroundGradient: [
    story({
      id: "background-gradient-basic",
      component: "BackgroundGradient",
      title: "BackgroundGradient",
      description: "Token gradient backdrop for mobile surfaces.",
      view: BackgroundGradient(
        { key: "background-gradient-basic", direction: "vertical", from: "background", to: "accent" },
        [Text({ key: "bg-label", content: "Gradient", variant: "title" })]
      )
    })
  ],
  Wallpaper: [
    story({
      id: "wallpaper-city",
      component: "Wallpaper",
      title: "Wallpaper",
      description: "Bounded wallpaper variant behind children.",
      view: Wallpaper({ key: "wallpaper-city", variant: "city" }, [
        Text({ key: "wall-label", content: "City wallpaper", variant: "body" })
      ])
    })
  ],
  Spotlight: [
    story({
      id: "spotlight-basic",
      component: "Spotlight",
      title: "Spotlight",
      description: "Focus glow treatment around a child slot.",
      view: Spotlight({ key: "spotlight-basic", intensity: "md" }, [
        Text({ key: "spot-label", content: "Focused", variant: "title" })
      ])
    })
  ],
  Frame: [
    story({
      id: "frame-khala-cut-corner",
      component: "Frame",
      title: "Frame · Khala cut corner",
      description: "Static inert cut-corner geometry around unchanged semantic content.",
      view: Frame(
        {
          key: "frame-khala-cut-corner",
          khala: {
            id: "gallery-frame-cut-corner",
            motif: "cut-corner-surface",
            width: 320,
            height: 120,
            density: "comfortable"
          }
        },
        [Text({ key: "frame-cut-label", content: "Framed", variant: "body" })]
      )
    }),
    story({
      id: "frame-khala-header-line",
      component: "Frame",
      title: "Frame · Khala header line",
      description: "Static inert heading accent with ordinary semantic content.",
      view: Frame(
        {
          key: "frame-khala-header-line",
          khala: {
            id: "gallery-frame-header-line",
            motif: "header-line",
            width: 320,
            height: 120,
            density: "compact"
          }
        },
        [Text({ key: "frame-header-label", content: "Runtime status", variant: "title" })]
      )
    }),
    story({
      id: "frame-khala-signal-separator",
      component: "Frame",
      title: "Frame · Khala signal separator",
      description: "Static inert separator using the same bounded Frame contract.",
      view: Frame(
        {
          key: "frame-khala-signal-separator",
          khala: {
            id: "gallery-frame-signal-separator",
            motif: "signal-separator",
            width: 320,
            height: 120,
            density: "spacious"
          }
        },
        [Text({ key: "frame-signal-label", content: "Live signal", variant: "body" })]
      )
    })
  ],
  BlurredPopup: [
    story({
      id: "blurred-popup-open",
      component: "BlurredPopup",
      title: "BlurredPopup",
      description: "Blur-backed popup on the overlay presence lifecycle.",
      view: BlurredPopup(
        {
          key: "blurred-popup-open",
          open: true,
          onDismiss: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
        },
        [Text({ key: "popup-label", content: "Long-press menu", variant: "body" })]
      )
    })
  ],
  IconButton: [
    story({
      id: "icon-button-glass",
      component: "IconButton",
      title: "IconButton",
      description: "Circular icon-only pressable over the closed icon set; glass surface variant.",
      view: Stack({ key: "icon-button-row", direction: "row", gap: "2", align: "center" }, [
        IconButton({
          key: "icon-button-plain",
          icon: "Play",
          accessibilityLabel: "Start",
          onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
        }),
        IconButton({
          key: "icon-button-glass",
          icon: "Reload",
          accessibilityLabel: "Reload",
          surface: "glass",
          onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
        }),
        IconButton({
          key: "icon-button-disabled",
          icon: "Stop",
          accessibilityLabel: "Stop",
          disabled: true,
          onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
        })
      ])
    })
  ],
  Toolbar: [
    story({
      id: "toolbar-glass",
      component: "Toolbar",
      title: "Toolbar",
      description: "Floating glass action strip with icon buttons.",
      view: Toolbar({ key: "toolbar-glass", placement: "bottom-floating", surface: "glass" }, [
        IconButton({
          key: "toolbar-play",
          icon: "Play",
          accessibilityLabel: "Start",
          onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
        }),
        IconButton({
          key: "toolbar-pause",
          icon: "Pause",
          accessibilityLabel: "Pause",
          onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
        }),
        Text({ key: "toolbar-hint", content: "Fleet ready", variant: "caption" })
      ])
    })
  ],
  EmptyMessage: [
    story({
      id: "empty-message",
      component: "EmptyMessage",
      title: "EmptyMessage",
      description: "Centered empty-state block: icon badge, title, description, and a typed Button action slot.",
      view: EmptyMessage({
        key: "empty-message",
        icon: { name: "Circle", tone: "secondary", size: "md" },
        title: "No sessions yet",
        description: "Start a new session to see it listed here.",
        action: Button({
          key: "empty-message-action",
          label: "New session",
          variant: "secondary",
          onPress: IntentRef("GalleryStory.Pressed", StaticPayload({ amount: 1 }))
        })
      }),
      controls: [
        enumControl("empty-message-icon-tone", "Icon tone", ["icon", "tone"], "secondary", emptyMessageIconTones),
        enumControl("empty-message-icon-size", "Icon size", ["icon", "size"], "md", emptyMessageIconSizes),
        textControl("empty-message-title", "Title", ["title"], "No sessions yet"),
        textControl(
          "empty-message-description",
          "Description",
          ["description"],
          "Start a new session to see it listed here."
        )
      ]
    })
  ],
  Avatar: [
    story({
      id: "avatar-fallback-chain",
      component: "Avatar",
      title: "Avatar",
      description: "Identity mark with the typed image -> initials -> icon fallback chain on the control lattice.",
      view: Stack({ key: "avatar-wrap", direction: "row", gap: "2", align: "center" }, [
        Avatar({
          key: "avatar-image",
          image: "https://example.com/assets/operator.png",
          initials: "OR",
          size: "lg",
          tone: "info",
          label: "Orrery"
        }),
        Avatar({
          key: "avatar-initials",
          initials: "WF",
          size: "lg",
          tone: "success",
          variant: "solid",
          label: "Whitefang"
        }),
        Avatar({ key: "avatar-icon", icon: "Circle", size: "lg", tone: "neutral" })
      ]),
      controls: [
        enumControl("avatar-size", "Size", ["children", 1, "size"], "lg", controlTokens),
        enumControl("avatar-tone", "Tone", ["children", 1, "tone"], "success", tones),
        enumControl("avatar-variant", "Variant", ["children", 1, "variant"], "solid", avatarVariants)
      ]
    })
  ],
  AvatarGroup: [
    story({
      id: "avatar-group-overflow",
      component: "AvatarGroup",
      title: "AvatarGroup",
      description: "Overlapping keyed avatars with a cutout ring and a +N overflow count past `max`.",
      view: AvatarGroup({
        key: "avatar-group",
        max: 3,
        size: "md",
        tone: "info",
        avatars: [
          keyed(Avatar({ key: "member-a", initials: "OR", label: "Orrery" })),
          keyed(Avatar({ key: "member-b", initials: "WF", label: "Whitefang" })),
          keyed(Avatar({ key: "member-c", icon: "Circle" })),
          keyed(Avatar({ key: "member-d", initials: "TR", label: "Trigger" }))
        ]
      }),
      controls: [
        enumControl("avatar-group-size", "Size", ["size"], "md", controlTokens),
        enumControl("avatar-group-tone", "Tone", ["tone"], "info", tones)
      ]
    })
  ],
  CopyButton: [
    story({
      id: "copy-button-shapes",
      component: "CopyButton",
      title: "CopyButton",
      description:
        "Typed copy-to-clipboard control (#84): icon-only lattice default, labelled shape, and controlled copied feedback.",
      view: Stack({ key: "copy-button-row", direction: "row", gap: "2", align: "center" }, [
        CopyButton({
          key: "copy-plain",
          content: "pnpm add effect",
          accessibilityLabel: "Copy install command",
          onCopy: IntentRef("GalleryStory.Changed", ComponentValueBinding())
        }),
        CopyButton({
          key: "copy-labelled",
          content: "diagnostics: all systems nominal",
          label: "Copy diagnostics",
          size: "lg",
          variant: "secondary",
          onCopy: IntentRef("GalleryStory.Changed", ComponentValueBinding())
        }),
        CopyButton({
          key: "copy-copied",
          content: "already copied",
          label: "Copy",
          copied: true,
          copiedLabel: "Copied"
        }),
        CopyButton({
          key: "copy-disabled",
          content: "unavailable",
          disabled: true
        })
      ])
    })
  ],
  Spinner: [
    story({
      id: "spinner-indeterminate",
      component: "Spinner",
      title: "Spinner",
      description:
        "Compact indeterminate in-flight ring on the control-lattice icon sub-token. " +
        "Determinate circular progress stays a Meter variant.",
      view: Spinner({ key: "spinner", size: "lg", tone: "info", label: "Loading" }),
      controls: [
        enumControl("spinner-size", "Size", ["size"], "lg", controlTokens),
        enumControl("spinner-tone", "Tone", ["tone"], "info", tones),
        booleanControl("spinner-reduce-motion", "Reduce motion", ["reduceMotion"], false)
      ]
    })
  ],
  LoadingDots: [
    story({
      id: "loading-dots-pulse",
      component: "LoadingDots",
      title: "LoadingDots",
      description: "3-dot pulse loading indicator on the control-lattice icon sub-token.",
      view: LoadingDots({ key: "dots", size: "lg", tone: "info", label: "Loading" }),
      controls: [
        enumControl("loading-dots-size", "Size", ["size"], "lg", controlTokens),
        enumControl("loading-dots-tone", "Tone", ["tone"], "info", tones),
        booleanControl("loading-dots-reduce-motion", "Reduce motion", ["reduceMotion"], false)
      ]
    })
  ],
  ShimmerText: [
    story({
      id: "shimmer-text-pending",
      component: "ShimmerText",
      title: "ShimmerText",
      description: "Shimmer sweep over real pending text — reduced motion keeps the text at a muted flat color.",
      view: ShimmerText({ key: "shimmer-pending", text: "Reading file…", label: "Reading file" }),
      controls: [
        textControl("shimmer-text-content", "Text", ["text"], "Reading file…"),
        booleanControl("shimmer-text-reduce-motion", "Reduce motion", ["reduceMotion"], false)
      ]
    }),
    story({
      id: "shimmer-text-skeleton",
      component: "ShimmerText",
      title: "ShimmerText (skeleton)",
      description: "Shimmer sweep over a skeleton placeholder width when no content has arrived yet.",
      view: ShimmerText({ key: "shimmer-skeleton", width: "sm" }),
      controls: [
        enumControl("shimmer-skeleton-width", "Width", ["width"], "sm", dimensionTokens),
        booleanControl("shimmer-skeleton-reduce-motion", "Reduce motion", ["reduceMotion"], false)
      ]
    })
  ]
} satisfies { readonly [Tag in ComponentTag]: ReadonlyArray<Story> }

export const generatedStoriesByComponent: {
  readonly [Tag in ComponentTag]: ReadonlyArray<Story>
} = componentStoryMap

export const defaultStorybook: Storybook = StorybookSchema.make({
  version: "effect-native/storybook/v0",
  title: "Effect Native component gallery",
  groups: componentTags.map((component) => ({
    component,
    title: component,
    stories: componentStoryMap[component]
  }))
})

export const allStories = (storybook: Storybook = defaultStorybook): ReadonlyArray<Story> =>
  storybook.groups.flatMap((group) => group.stories)

export const storiesForComponent = (
  component: ComponentTag,
  storybook: Storybook = defaultStorybook
): ReadonlyArray<Story> => storybook.groups.find((group) => group.component === component)?.stories ?? []

export const storyById = (storyId: string, storybook: Storybook = defaultStorybook): Story | undefined =>
  allStories(storybook).find((candidate) => candidate.id === storyId)

export const storyCoverage = (
  storybook: Storybook = defaultStorybook
): { readonly missing: ReadonlyArray<ComponentTag>; readonly covered: ReadonlyArray<ComponentTag> } => {
  const covered = new Set(storybook.groups.filter((group) => group.stories.length > 0).map((group) => group.component))
  return {
    covered: componentTags.filter((tag) => covered.has(tag)),
    missing: componentTags.filter((tag) => !covered.has(tag))
  }
}

export const proofScreenBaselineComponents = [
  "Stack",
  "Text",
  "Button",
  "Image",
  "TextField",
  "List",
  "Card",
  "Spacer"
] as const satisfies ReadonlyArray<ComponentTag>

export const proofScreenBaselineStories = proofScreenBaselineComponents.map((component) => {
  const storyItem = storiesForComponent(component)[0]
  if (storyItem === undefined) {
    throw new Error(`Missing proof-screen baseline story for ${component}`)
  }
  return storyItem
})

// ---------------------------------------------------------------------------
// Documentation pages (issue #87).
//
// Mirrors the Apps SDK UI Storybook docs *discipline* — one docs page per
// catalog component rendering its live variants, plus foundation pages for
// tokens/colors/typography/icons/responsive — without any Storybook or MDX
// tooling. Every page is typed Effect Native view data rendered by the same
// gallery app, so agents and humans can answer "what exists, what are its
// variants" without reading source.
// ---------------------------------------------------------------------------

/**
 * One-line documentation summary per catalog component. The `satisfies`
 * constraint is the compile-time half of the completeness gate: a new
 * componentTag fails the gallery build until it is documented here (the
 * runtime half is `galleryPageCoverage` + the gallery test suite).
 */
export const componentPageSummaries = {
  Stack: "Flex layout primitive: typed direction, gap, alignment, and responsive direction/spacing per breakpoint.",
  Text: "Typography primitive over the closed type scale (caption/body/label/title/heading) with token colors and weights.",
  Button: "Pressable action with variants primary / secondary / ghost, disabled state, and a typed onPress intent.",
  Image: "Media box with fit contain / cover / fill, alt text, and token radius.",
  TextField:
    "Single- or multi-line text input with label, placeholder, secure mode, focus state, and typed change/submit intents; opt-in variant/size/gutterSize matrix box chrome, invalid cue, and multiline autoResize (harmonization #79).",
  List: "Keyed vertical collection with optional virtualization and estimated item size.",
  SectionList: "Grouped keyed collection with sticky headers and optional virtualization.",
  Card: "Bordered surface container with token padding and radius.",
  Spacer: "Layout gap: fixed token size or flex to absorb remaining space.",
  Link: "Typed navigation to a path or anchor destination.",
  Modal: "Centered dialog with typed size, open state, and dismiss intent.",
  Sheet: "Edge-anchored panel (bottom / side) with detents and dismiss intent.",
  Host: "Foreign-host escape hatch: serializable kind + props, driver-owned widget.",
  Icon: "Closed icon-name registry rendered at token sizes with token colors.",
  Divider: "Horizontal or vertical separator line.",
  Badge:
    "Status/count pill over the closed tone set; opt-in tone x variant x size matrix chrome via variant/size (harmonization #79).",
  Chip: "Label + value pill for dense status strips; opt-in tone x variant x size matrix chrome via variant/size (harmonization #79).",
  Meter: "Determinate progress/capacity readout with tone.",
  StatTile: "Label + strong value summary cell.",
  Table: "Typed columns and keyed rows of arbitrary cell views.",
  SplitPane: "Resizable split layout with typed pane sizes and resize/collapse intents.",
  NavRail: "Sectioned navigation rail with selectable items and a typed selection intent.",
  Workbench: "Named panes with typed active-pane swapping — no mount/unmount closures.",
  Popover: "Anchored floating surface with typed placement and dismiss intent.",
  DropdownMenu: "Keyboard-navigable menu from a typed item model.",
  ContextMenu: "Pointer-anchored menu at a typed position, same item model as DropdownMenu.",
  Tooltip: "Hover/focus label wrapping exactly one target.",
  Combobox: "Typeahead input with app-supplied options, highlight state, and typed intents.",
  CommandPalette: "Modal overlay composing a Combobox on the presence primitive.",
  Tabs: "WAI-ARIA tablist with typed tab items, badges, and panel association by id.",
  Composer: "Chat input over a typed document model with mentions, attachments, and slash/mention autocomplete.",
  Toggle: "Boolean switch with typed value and onChange intent.",
  Select:
    "Single choice from a typed option list; opt-in trigger variant/size/pill/dropdownIcon chrome, and additive multi-select via multiple/values (harmonization #79).",
  Checkbox: "Multi-select boolean with typed checked state.",
  RadioGroup: "Exclusive choice group with typed value.",
  SegmentedControl: "Single-choice control with an animated selection thumb (DOM); lattice size, gutterSize, and pill.",
  Slider: "Bounded numeric range input with step.",
  NumberField: "Bounded numeric input with step/min/max.",
  FieldRow: "Label + control + description + error layout for settings panels.",
  Toast: "Single transient notification with live region and dismiss.",
  ToastRegion: "Stacked, placement-aware notification region.",
  StatusBanner: "Persistent inline banner with typed tone plus retry/dismiss intents.",
  Alert:
    "Icon + title + body callout on the tone x variant matrix (harmonization #79) — distinct from StatusBanner's persistent single-line banner role.",
  RecoveryOverlay: "Full-surface blocking overlay with typed recovery actions.",
  Markdown: "Pre-parsed typed block+inline document model — no parser, no raw HTML.",
  Transcript: "Append-optimized log of role-styled messages with typed status.",
  CodeBlock: "Pre-tokenized code lines with theme syntax colors and line numbers.",
  DiffView: "Pre-parsed unified diff with add/remove theming and per-line review intents.",
  GraphFigure: "Typed node/edge graph over the canvas renderer with DOM/SVG fallback.",
  Timeline: "Run timeline of typed events.",
  Section: "Marketing layout band with contained width and vertical padding.",
  Hero: "Display-scale headline, subhead, CTA row, and optional media slot.",
  AnnouncementBadge: "Outlined pill above a hero with optional action intent.",
  CtaSection: "Headline, supporting copy, and action row for mid-page conversion.",
  Footer: "Brand slot, typed link columns, and legal row.",
  NavBar: "Marketing top navigation with brand, links, actions, and collapse toggle.",
  Accordion: "Disclosure list with typed expanded ids and toggle intent.",
  PricingColumn: "Named plan, price, feature list, and CTA intent.",
  PricingTable: "Side-by-side pricing columns.",
  LogoRow: "Trusted-by logo strip.",
  StatsBand: "Metric band of values and labels.",
  Glow: "Bounded radial accent glow behind a child slot.",
  MockupFrame: "Browser/device frame with optional perspective tilt.",
  Pager: "Linear stepper with progress dots and typed advance/complete intents.",
  SwipeableListItem: "List row with typed leading/trailing swipe actions.",
  BackgroundGradient: "Token gradient backdrop.",
  Wallpaper: "Bounded wallpaper variant behind children.",
  Spotlight: "Focus glow treatment around a child slot.",
  Frame: "Decorative bordered frame around content.",
  BlurredPopup: "Blur-backed popup on the overlay presence lifecycle.",
  IconButton: "Circular icon-only pressable over the closed icon set with glass surface variant.",
  Toolbar: "Floating action strip (glass surface) hosting icon buttons.",
  EmptyMessage:
    "Centered empty-state block: icon badge (bounded tone/size), title, description, and a typed Button action slot.",
  Avatar:
    "Identity mark with the typed image -> initials -> icon fallback chain, lattice size, and tone soft/solid variants.",
  AvatarGroup: "Overlapping keyed avatars with a cutout ring and a +N overflow count past max.",
  CopyButton:
    "Copy-to-clipboard control over the injected Clipboard service: icon-only lattice default, labelled shape, copied-state feedback, typed onCopy intent.",
  Spinner:
    "Compact indeterminate in-flight ring on the lattice icon sub-token; determinate progress stays a Meter variant.",
  LoadingDots: "3-dot pulse loading indicator on the lattice icon sub-token.",
  ShimmerText:
    "Shimmer sweep over real pending text or a skeleton placeholder width; reduced motion is a static affordance."
} as const satisfies Record<ComponentTag, string>

export const foundationPageIds = ["khala-ui", "design-tokens", "colors", "typography", "icons", "responsive"] as const
export type FoundationPageId = (typeof foundationPageIds)[number]

export type GalleryPageKind = "component" | "foundation"

export interface GalleryPage {
  readonly id: string
  readonly title: string
  readonly kind: GalleryPageKind
  readonly description: string
  readonly view: View
}

export const componentPageId = (tag: ComponentTag): string => `component:${tag}`

const pageSectionTitle = (key: string, content: string): View =>
  Text({ key, content, variant: "title", color: "textPrimary", style: { marginTop: "2" } })

const pageCaption = (key: string, content: string): View =>
  Text({ key, content, variant: "caption", color: "textMuted" })

/**
 * Enumerated variant facts for a story: every enum/token control is listed
 * with its full option set so a page answers "what variants exist" in text,
 * not just pixels.
 */
const storyVariantFacts = (storyItem: Story): ReadonlyArray<View> =>
  storyItem.controls
    .filter((control) => (control.options?.length ?? 0) > 0)
    .map((control) =>
      pageCaption(
        `page-facts-${storyItem.id}-${control.id}`,
        `${control.label}: ${(control.options ?? []).join(" / ")}`
      )
    )

const componentPageView = (tag: ComponentTag): View => {
  const stories = componentStoryMap[tag]
  return Stack({ key: `page-component-${tag}`, direction: "column", gap: "4" }, [
    Stack({ key: `page-component-${tag}-head`, direction: "column", gap: "1" }, [
      Text({ key: `page-component-${tag}-title`, content: tag, variant: "heading", color: "textPrimary" }),
      Text({
        key: `page-component-${tag}-summary`,
        content: componentPageSummaries[tag],
        variant: "body",
        color: "textMuted"
      }),
      pageCaption(
        `page-component-${tag}-count`,
        `${stories.length} ${stories.length === 1 ? "story" : "stories"} · live from the typed story fixtures`
      )
    ]),
    ...stories.map((storyItem) =>
      Card(
        {
          key: `page-story-${storyItem.id}`,
          padding: "4",
          radius: "lg",
          style: { backgroundColor: "surface", borderColor: "border", borderWidth: 1 }
        },
        [
          Stack({ key: `page-story-${storyItem.id}-stack`, direction: "column", gap: "2" }, [
            Stack({ key: `page-story-${storyItem.id}-head`, direction: "row", gap: "2", align: "center" }, [
              Text({
                key: `page-story-${storyItem.id}-title`,
                content: storyItem.title,
                variant: "title",
                color: "textPrimary"
              }),
              Badge({
                key: `page-story-${storyItem.id}-kind`,
                label: storyItem.kind,
                tone: storyItem.kind === "hand-authored" ? "info" : "neutral"
              })
            ]),
            Text({
              key: `page-story-${storyItem.id}-description`,
              content: storyItem.description,
              variant: "body",
              color: "textMuted"
            }),
            ...storyVariantFacts(storyItem),
            Card(
              {
                key: `page-story-${storyItem.id}-canvas`,
                padding: "4",
                radius: "md",
                style: { backgroundColor: "background", borderColor: "borderSubtle", borderWidth: 1 }
              },
              [storyItem.view]
            )
          ])
        ]
      )
    )
  ])
}

const designTokensPageView: View = Stack({ key: "page-design-tokens", direction: "column", gap: "3" }, [
  Text({ key: "page-design-tokens-title", content: "Design tokens", variant: "heading", color: "textPrimary" }),
  Text({
    key: "page-design-tokens-summary",
    content:
      "Every themable value in Effect Native is a closed token scale resolved through the single khalaTheme. Values below are the live khalaTheme numbers.",
    variant: "body",
    color: "textMuted"
  }),
  pageSectionTitle("page-tokens-spacing-title", `Spacing (${spacingTokens.length} steps)`),
  ...spacingTokens.map((token) =>
    Stack({ key: `page-spacing-${token}`, direction: "row", gap: "3", align: "center" }, [
      Text({
        key: `page-spacing-${token}-name`,
        content: token,
        variant: "label",
        color: "textPrimary",
        style: { width: 64 }
      }),
      Card({
        key: `page-spacing-${token}-bar`,
        radius: "sm",
        style: { width: khalaTheme.spacing[token], height: 8, backgroundColor: "accent" }
      }),
      pageCaption(`page-spacing-${token}-value`, `${khalaTheme.spacing[token]}px`)
    ])
  ),
  pageSectionTitle("page-tokens-radius-title", `Radius (${radiusTokens.length} steps)`),
  ...radiusTokens.map((token) =>
    Stack({ key: `page-radius-${token}`, direction: "row", gap: "3", align: "center" }, [
      Text({
        key: `page-radius-${token}-name`,
        content: token,
        variant: "label",
        color: "textPrimary",
        style: { width: 64 }
      }),
      Card({
        key: `page-radius-${token}-swatch`,
        style: {
          width: 48,
          height: 28,
          backgroundColor: "surfaceRaised",
          borderColor: "borderStrong",
          borderWidth: 1,
          borderRadius: token
        }
      }),
      pageCaption(`page-radius-${token}-value`, `${khalaTheme.radius[token]}px`)
    ])
  ),
  pageSectionTitle("page-tokens-dimension-title", `Dimension (${dimensionTokens.length} steps)`),
  ...dimensionTokens.map((token) =>
    Stack({ key: `page-dimension-${token}`, direction: "row", gap: "3", align: "center" }, [
      Text({
        key: `page-dimension-${token}-name`,
        content: token,
        variant: "label",
        color: "textPrimary",
        style: { width: 64 }
      }),
      pageCaption(
        `page-dimension-${token}-value`,
        typeof khalaTheme.dimension[token] === "number"
          ? `${khalaTheme.dimension[token]}px`
          : String(khalaTheme.dimension[token])
      )
    ])
  ),
  pageSectionTitle("page-tokens-control-title", `Control lattice (${controlTokens.length} sizes)`),
  ...controlTokens.map((token) =>
    Stack({ key: `page-control-${token}`, direction: "row", gap: "3", align: "center" }, [
      Text({
        key: `page-control-${token}-name`,
        content: token,
        variant: "label",
        color: "textPrimary",
        style: { width: 64 }
      }),
      pageCaption(
        `page-control-${token}-value`,
        `height ${khalaTheme.control[token].height}px · gutter ${khalaTheme.control[token].gutter}px · icon ${khalaTheme.control[token].icon}px`
      )
    ])
  ),
  pageSectionTitle("page-tokens-motion-title", "Motion"),
  pageCaption(
    "page-motion-durations",
    `durations: fast ${khalaTheme.motion.durationFastMs}ms · enter ${khalaTheme.motion.durationEnterMs}ms · exit ${khalaTheme.motion.durationExitMs}ms`
  ),
  pageCaption("page-motion-ease-basic", `easeBasic: ${khalaTheme.motion.easeBasic}`),
  pageCaption("page-motion-ease-enter", `easeEnter: ${khalaTheme.motion.easeEnter}`),
  pageCaption("page-motion-ease-exit", `easeExit: ${khalaTheme.motion.easeExit}`),
  pageSectionTitle("page-tokens-elevation-title", "Elevation"),
  pageCaption("page-elevation-shadow", `overlayShadow: ${khalaTheme.elevation.overlayShadow}`),
  pageCaption("page-elevation-hairline", `hairlineWidth: ${khalaTheme.elevation.hairlineWidth}px`)
])

const colorsPageView: View = Stack({ key: "page-colors", direction: "column", gap: "3" }, [
  Text({ key: "page-colors-title", content: "Colors", variant: "heading", color: "textPrimary" }),
  Text({
    key: "page-colors-summary",
    content: `All ${colorTokens.length} semantic color roles. Swatches render live under the mounted theme; hex values are the khalaTheme (Protoss blue, dark-only) assignments.`,
    variant: "body",
    color: "textMuted"
  }),
  ...colorTokens.map((name) =>
    Stack({ key: `page-color-${name}`, direction: "row", gap: "3", align: "center" }, [
      Card({
        key: `page-color-${name}-swatch`,
        radius: "md",
        style: {
          width: 56,
          height: 28,
          backgroundColor: name,
          borderColor: "borderSubtle",
          borderWidth: 1
        }
      }),
      Text({
        key: `page-color-${name}-name`,
        content: name,
        variant: "label",
        color: "textPrimary",
        style: { width: 160 }
      }),
      pageCaption(`page-color-${name}-value`, khalaTheme.color[name])
    ])
  )
])

const typographyPageView: View = Stack({ key: "page-typography", direction: "column", gap: "4" }, [
  Text({ key: "page-typography-title", content: "Typography", variant: "heading", color: "textPrimary" }),
  Text({
    key: "page-typography-summary",
    content: `The closed ${typeScaleTokens.length}-step type scale. Specimens render live; metrics are the khalaTheme values.`,
    variant: "body",
    color: "textMuted"
  }),
  ...typeScaleTokens.map((variant) =>
    Stack({ key: `page-type-${variant}`, direction: "column", gap: "1" }, [
      Text({
        key: `page-type-${variant}-specimen`,
        content: "The quick brown fox jumps over the lazy dog",
        variant,
        color: "textPrimary"
      }),
      pageCaption(
        `page-type-${variant}-meta`,
        `${variant} — ${khalaTheme.typeScale[variant].fontSize}px / ${khalaTheme.typeScale[variant].lineHeight}px line height / weight ${khalaTheme.typeScale[variant].fontWeight}`
      )
    ])
  )
])

const iconsPageView: View = Stack({ key: "page-icons", direction: "column", gap: "3" }, [
  Text({ key: "page-icons-title", content: "Icons", variant: "heading", color: "textPrimary" }),
  Text({
    key: "page-icons-summary",
    content: `The full closed icon registry: ${iconNames.length} names at sizes ${iconSizes.join(" / ")}. Color comes from the token system.`,
    variant: "body",
    color: "textMuted"
  }),
  ...iconNames.map((name) =>
    Stack({ key: `page-icon-${name}`, direction: "row", gap: "3", align: "center" }, [
      ...iconSizes.map((size) =>
        Icon({
          key: `page-icon-${name}-${size}`,
          name,
          size,
          color: "textPrimary",
          label: `${name} (${size})`
        })
      ),
      Text({ key: `page-icon-${name}-name`, content: name, variant: "label", color: "textPrimary" })
    ])
  )
])

const responsivePageView: View = Stack({ key: "page-responsive", direction: "column", gap: "3" }, [
  Text({ key: "page-responsive-title", content: "Responsive", variant: "heading", color: "textPrimary" }),
  Text({
    key: "page-responsive-summary",
    content:
      "Responsive behavior is typed data: any responsive prop takes per-breakpoint values ({ base, sm, md, lg, xl }) resolved against the theme breakpoints below.",
    variant: "body",
    color: "textMuted"
  }),
  pageSectionTitle("page-responsive-breakpoints-title", `Breakpoints (${breakpointTokens.length})`),
  ...breakpointTokens.map((token) =>
    Stack({ key: `page-breakpoint-${token}`, direction: "row", gap: "3", align: "center" }, [
      Text({
        key: `page-breakpoint-${token}-name`,
        content: token,
        variant: "label",
        color: "textPrimary",
        style: { width: 64 }
      }),
      pageCaption(`page-breakpoint-${token}-value`, `${khalaTheme.breakpoint[token]}px and up`)
    ])
  ),
  pageSectionTitle("page-responsive-viewports-title", "Gallery preview viewports"),
  ...galleryViewports.map((entry) =>
    Stack({ key: `page-viewport-${entry.id}`, direction: "row", gap: "3", align: "center" }, [
      Text({
        key: `page-viewport-${entry.id}-name`,
        content: entry.label,
        variant: "label",
        color: "textPrimary",
        style: { width: 64 }
      }),
      pageCaption(`page-viewport-${entry.id}-value`, `${entry.viewport.width} × ${entry.viewport.height}`)
    ])
  ),
  pageSectionTitle("page-responsive-demo-title", "Live demo: column below md, row at md and up"),
  Stack(
    {
      key: "page-responsive-demo",
      direction: { base: "column", md: "row" },
      gap: { base: "2", md: "4" },
      padding: "3",
      style: {
        borderColor: "border",
        borderWidth: 1,
        variants: { breakpoint: { md: { backgroundColor: "surface" } } }
      }
    },
    [
      Card({ key: "page-responsive-demo-a", padding: "3", radius: "md" }, [
        Text({ key: "page-responsive-demo-a-copy", content: "Mobile first", variant: "body", color: "textPrimary" })
      ]),
      Card({ key: "page-responsive-demo-b", padding: "3", radius: "md" }, [
        Text({ key: "page-responsive-demo-b-copy", content: "Desktop row", variant: "body", color: "textPrimary" })
      ])
    ]
  )
])

const khalaUiContractPageView: View = Stack({ key: "page-khala-ui", direction: "column", gap: "4" }, [
  Text({ key: "page-khala-ui-title", content: "Khala UI contract", variant: "heading", color: "textPrimary" }),
  Text({
    key: "page-khala-ui-summary",
    content:
      "Khala UI is the owned OpenAgents visual language inside Effect Native. It is not a component runtime, state store, intent system, React library, or second theme.",
    variant: "body",
    color: "textMuted"
  }),
  Alert({
    key: "page-khala-ui-authority",
    tone: "info",
    variant: "soft",
    title: "Authority stays singular",
    message:
      "Effect Native owns components, views, intents, and lifecycle. @effect-native/tokens and khalaTheme remain the only theme authority."
  }),
  pageSectionTitle("page-khala-ui-vocabulary-title", "Complete static vocabulary: twelve motifs"),
  ...khalaUiMotifIds.map((motif) =>
    Card({ key: `page-khala-ui-motif-${motif}`, padding: "3", radius: "md" }, [
      Stack({ key: `page-khala-ui-motif-${motif}-content`, direction: "column", gap: "1" }, [
        Text({ key: `page-khala-ui-motif-${motif}-name`, content: motif, variant: "title", color: "textPrimary" }),
        pageCaption(
          `page-khala-ui-motif-${motif}-rule`,
          motif === "cut-corner-surface"
            ? "One restrained surface-edge treatment; semantic content remains the existing component."
            : motif === "header-line"
              ? "One short structural line that reinforces a section heading without gating it."
              : motif === "signal-separator"
                ? "One separator for hierarchy or live state; never a semantic signal by itself."
                : "Owned bounded frame geometry; decoration stays inert and yields before semantic content."
        )
      ])
    ])
  ),
  pageSectionTitle("page-khala-ui-restraint-title", "Density and nesting limits"),
  pageCaption(
    "page-khala-ui-restraint-numbers",
    `one signature frame per region · max decorated nesting ${khalaUiRestraintLimits.maxDecoratedSurfaceNesting} · max motifs per surface ${khalaUiRestraintLimits.maxMotifsPerSurface} · focus clearance ${khalaUiRestraintLimits.minFocusClearancePx}px`
  ),
  pageCaption("page-khala-ui-restraint-compact", `compact: ${khalaUiRestraintLimits.compact}`),
  pageCaption("page-khala-ui-restraint-comfortable", `comfortable: ${khalaUiRestraintLimits.comfortable}`),
  pageCaption("page-khala-ui-restraint-spacious", `spacious: ${khalaUiRestraintLimits.spacious}`),
  pageSectionTitle("page-khala-ui-capabilities-title", "Renderer capability dispositions"),
  ...khalaUiMotifIds.flatMap((motif) =>
    khalaUiRendererIds.map((renderer) => {
      const capability = khalaUiCapabilityMatrix[motif][renderer]
      return Stack(
        { key: `page-khala-ui-capability-${motif}-${renderer}`, direction: "row", gap: "2", align: "center" },
        [
          Text({
            key: `page-khala-ui-capability-${motif}-${renderer}-label`,
            content: `${motif} / ${renderer}`,
            variant: "label",
            color: "textPrimary",
            style: { width: 280 }
          }),
          Badge({
            key: `page-khala-ui-capability-${motif}-${renderer}-status`,
            label: capability.disposition,
            tone:
              capability.disposition === "supported"
                ? "success"
                : capability.disposition === "degraded"
                  ? "warn"
                  : "neutral"
          }),
          pageCaption(`page-khala-ui-capability-${motif}-${renderer}-reason`, capability.rationale)
        ]
      )
    })
  ),
  pageSectionTitle("page-khala-ui-golden-title", "Golden fixtures: static renderer proofs passing"),
  ...khalaUiGoldenFixtures.map((fixture) =>
    Card({ key: `page-khala-ui-golden-${fixture.id}`, padding: "4", radius: "lg" }, [
      Stack({ key: `page-khala-ui-golden-${fixture.id}-content`, direction: "column", gap: "3" }, [
        Stack({ key: `page-khala-ui-golden-${fixture.id}-heading`, direction: "row", gap: "2", align: "center" }, [
          Text({
            key: `page-khala-ui-golden-${fixture.id}-title`,
            content: fixture.motif,
            variant: "title",
            color: "textPrimary"
          }),
          Badge({
            key: `page-khala-ui-golden-${fixture.id}-density`,
            label: fixture.density,
            tone: "neutral"
          })
        ]),
        fixture.decoratedView,
        Alert({
          key: `page-khala-ui-golden-${fixture.id}-geometry`,
          tone: "success",
          variant: "outline",
          title: `${fixture.geometryProof._tag} proof · ${fixture.geometryProof.owner}`,
          message: fixture.geometryProof.receipt
        }),
        Alert({
          key: `page-khala-ui-golden-${fixture.id}-renderer`,
          tone: "success",
          variant: "outline",
          title: `${fixture.decorationProof._tag} renderer proof · ${fixture.decorationProof.owner}`,
          message: fixture.decorationProof.receipt
        })
      ])
    ])
  ),
  pageSectionTitle("page-khala-ui-proof-title", "KU-2 / KU-3 proof matrix"),
  ...khalaUiProofCases.map((proof) =>
    Stack({ key: `page-khala-ui-proof-${proof.id}`, direction: "row", gap: "2", align: "center" }, [
      Text({
        key: `page-khala-ui-proof-${proof.id}-label`,
        content: proof.id,
        variant: "label",
        color: "textPrimary",
        style: { width: 240 }
      }),
      Badge({
        key: `page-khala-ui-proof-${proof.id}-status`,
        label: `${proof.status} · ${proof.owner}`,
        tone: proof.status === "passing" ? "success" : "warn"
      }),
      pageCaption(`page-khala-ui-proof-${proof.id}-expectation`, proof.expectation)
    ])
  ),
  pageSectionTitle("page-khala-ui-budget-title", "Performance budgets"),
  pageCaption(
    "page-khala-ui-budget-static",
    `static delta ≤ ${khalaUiPerformanceBudgets.staticBundleGzipBytes} gzip bytes · ≤ ${khalaUiPerformanceBudgets.maxDecorativeNodesPerMotif} inert nodes per motif · zero static schedulers/timers/observers/layout reads`
  ),
  pageCaption(
    "page-khala-ui-budget-receipt",
    "measured combined KU-2/KU-3 delta passes the 8 KiB gzip gate · see the static renderer receipt"
  ),
  pageCaption(
    "page-khala-ui-budget-canvas",
    `future Canvas: ≤ ${khalaUiPerformanceBudgets.maxCanvasSurfacesPerProductRegion} surface per region · DPR ≤ ${khalaUiPerformanceBudgets.maxCanvasDevicePixelRatio} · p95 work ≤ ${khalaUiPerformanceBudgets.maxCanvasFrameWorkMsP95}ms · memory ≤ ${khalaUiPerformanceBudgets.maxCanvasMemoryMiB}MiB`
  ),
  pageSectionTitle("page-khala-ui-provenance-title", "Arwes provenance boundary"),
  pageCaption(
    "page-khala-ui-provenance",
    `reference ${khalaUiArwesReference.commit} · ${khalaUiArwesReference.license} · behavior studied, source adapted: ${String(khalaUiArwesReference.sourceAdapted)}`
  ),
  Alert({
    key: "page-khala-ui-sound-prohibition",
    tone: "warning",
    variant: "outline",
    title: "Sound assets prohibited",
    message:
      "The Arwes website sound assets are website-only and must never be copied or redistributed by Effect Native or OpenAgents."
  })
])

const foundationPageViews: Record<FoundationPageId, View> = {
  "khala-ui": khalaUiContractPageView,
  "design-tokens": designTokensPageView,
  colors: colorsPageView,
  typography: typographyPageView,
  icons: iconsPageView,
  responsive: responsivePageView
}

const foundationPageMeta: Record<FoundationPageId, { readonly title: string; readonly description: string }> = {
  "khala-ui": {
    title: "Khala UI",
    description:
      "Language authority, three-motif vocabulary, restraint limits, renderer dispositions, golden fixtures, proof slots, budgets, and provenance."
  },
  "design-tokens": {
    title: "Design tokens",
    description:
      "Spacing, radius, dimension, control-lattice, motion, and elevation scales with live khalaTheme values."
  },
  colors: {
    title: "Colors",
    description: "The full semantic color-role matrix with live swatches and khalaTheme hex values."
  },
  typography: {
    title: "Typography",
    description: "Type-scale specimens with khalaTheme font metrics."
  },
  icons: {
    title: "Icons",
    description: "The full closed icon registry with names and every token size."
  },
  responsive: {
    title: "Responsive",
    description: "Breakpoint tokens, gallery viewports, and a live responsive layout demo."
  }
}

export const foundationPages: ReadonlyArray<GalleryPage> = foundationPageIds.map((id) => ({
  id,
  title: foundationPageMeta[id].title,
  kind: "foundation" as const,
  description: foundationPageMeta[id].description,
  view: foundationPageViews[id]
}))

export const componentPages: ReadonlyArray<GalleryPage> = componentTags.map((tag) => ({
  id: componentPageId(tag),
  title: tag,
  kind: "component" as const,
  description: componentPageSummaries[tag],
  view: componentPageView(tag)
}))

export const galleryPages: ReadonlyArray<GalleryPage> = [...foundationPages, ...componentPages]

export const galleryPageById = (pageId: string): GalleryPage | undefined =>
  galleryPages.find((page) => page.id === pageId)

/**
 * Mechanical completeness check: every catalog componentTag must have a
 * component docs page with a non-empty summary and at least one live story.
 * The gallery test suite asserts `missing` is empty, so future components
 * fail the gallery until they are documented.
 */
export const galleryPageCoverage = (): {
  readonly missing: ReadonlyArray<ComponentTag>
  readonly covered: ReadonlyArray<ComponentTag>
} => {
  const documented = new Set(
    galleryPages
      .filter((page) => page.kind === "component" && page.description.trim().length > 0)
      .map((page) => page.id)
  )
  const hasPage = (tag: ComponentTag): boolean =>
    documented.has(componentPageId(tag)) && storiesForComponent(tag).length > 0
  return {
    covered: componentTags.filter(hasPage),
    missing: componentTags.filter((tag) => !hasPage(tag))
  }
}

export const serializeStory = (input: Story): string => JSON.stringify(Schema.encodeSync(StorySchema)(input), null, 2)

export const parseStory = (source: string): Story => Schema.decodeUnknownSync(StorySchema)(JSON.parse(source))

export const serializeStorybook = (input: Storybook): string =>
  JSON.stringify(Schema.encodeSync(StorybookSchema)(input), null, 2)

export const parseStorybook = (source: string): Storybook =>
  Schema.decodeUnknownSync(StorybookSchema)(JSON.parse(source))

const setAtPath = (value: unknown, path: ReadonlyArray<StoryPathSegment>, next: JsonPayload): unknown => {
  if (path.length === 0) {
    return next
  }

  const [segment, ...rest] = path
  if (Array.isArray(value)) {
    const index = typeof segment === "number" ? segment : Number(segment)
    return value.map((item, itemIndex) => (itemIndex === index ? setAtPath(item, rest, next) : item))
  }

  const object = typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {}
  return {
    ...object,
    [String(segment)]: setAtPath(object[String(segment)], rest, next)
  }
}

export const applyStoryControlValue = (input: Story, controlId: string, value: JsonPayload): Story => {
  const control = input.controls.find((candidate) => candidate.id === controlId)
  if (control === undefined) {
    return input
  }
  return StorySchema.make({
    ...input,
    controls: input.controls.map((candidate) => (candidate.id === controlId ? { ...candidate, value } : candidate)),
    view: Schema.decodeUnknownSync(ViewSchema)(setAtPath(input.view, control.path, value))
  })
}

export const applyStoryControlValues = (input: Story, values: Readonly<Record<string, JsonPayload>>): Story =>
  Object.entries(values).reduce(
    (current, [controlId, value]) => applyStoryControlValue(current, controlId, value),
    input
  )

export const StorySelected = defineIntent("Gallery.StorySelected", Schema.NonEmptyString)
export const ComponentSelected = defineIntent("Gallery.ComponentSelected", Schema.Literals(componentTags))
export const ControlValueChanged = defineIntent(
  "Gallery.ControlValueChanged",
  Schema.Struct({
    controlId: Schema.NonEmptyString,
    value: JsonPayloadSchema
  })
)
export const ThemeSelected = defineIntent("Gallery.ThemeSelected", Schema.Literals(["default", "dark"] as const))
export const ViewportSelected = defineIntent(
  "Gallery.ViewportSelected",
  Schema.Literals(["phone", "tablet", "desktop"] as const)
)
export const SerializedStoryChanged = defineIntent("Gallery.SerializedStoryChanged", Schema.String)
/** Select a documentation page by id; the empty string returns to the story browser. */
export const PageSelected = defineIntent("Gallery.PageSelected", Schema.String)

export const galleryIntentDefinitions = [
  StorySelected,
  ComponentSelected,
  ControlValueChanged,
  ThemeSelected,
  ViewportSelected,
  SerializedStoryChanged,
  PageSelected
] as const

export const GalleryStateSchema = Schema.Struct({
  storybook: StorybookSchema,
  activeComponent: Schema.Literals(componentTags),
  activeStoryId: Schema.NonEmptyString,
  activeThemeId: Schema.Literals(["default", "dark"] as const),
  activeViewportId: Schema.Literals(["phone", "tablet", "desktop"] as const),
  /** Active documentation page id; the empty string means the story browser. */
  activePageId: Schema.String,
  controlValues: Schema.Record(Schema.String, Schema.Record(Schema.String, JsonPayloadSchema)),
  pressedCount: Schema.Number,
  changedValue: Schema.String,
  submittedValue: Schema.String,
  dismissedSurface: Schema.String
})
export type GalleryState = Schema.Schema.Type<typeof GalleryStateSchema>

export const initialGalleryState = (storybook: Storybook = defaultStorybook): GalleryState => {
  const firstGroup = storybook.groups[0]
  const firstStory = firstGroup?.stories[0]
  if (firstGroup === undefined || firstStory === undefined) {
    throw new Error("Gallery storybook requires at least one story")
  }
  return GalleryStateSchema.make({
    storybook,
    activeComponent: firstGroup.component,
    activeStoryId: firstStory.id,
    activeThemeId: "default",
    activeViewportId: "desktop",
    activePageId: "",
    controlValues: {},
    pressedCount: 0,
    changedValue: "",
    submittedValue: "",
    dismissedSurface: ""
  })
}

const activeTheme = (state: GalleryState) =>
  galleryThemes.find((theme) => theme.id === state.activeThemeId) ?? galleryThemes[0]

const activeViewport = (state: GalleryState) =>
  galleryViewports.find((viewport) => viewport.id === state.activeViewportId) ?? galleryViewports[2]

export const activeStory = (state: GalleryState): Story => {
  const selected = storyById(state.activeStoryId, state.storybook)
  if (selected !== undefined) {
    return applyStoryControlValues(selected, state.controlValues[selected.id] ?? {})
  }
  const fallback = state.storybook.groups[0]?.stories[0]
  if (fallback === undefined) {
    throw new Error("Gallery storybook requires at least one story")
  }
  return fallback
}

const selectedStoryGroup = (state: GalleryState): StoryGroup =>
  state.storybook.groups.find((group) => group.component === state.activeComponent) ?? state.storybook.groups[0]!

const controlValue = (state: GalleryState, story: Story, control: StoryControl): JsonPayload =>
  state.controlValues[story.id]?.[control.id] ?? control.value

const optionButtons = (story: Story, control: StoryControl, current: JsonPayload): ReadonlyArray<View> =>
  (control.options ?? []).map((option) =>
    Button({
      key: `${control.id}-${option}`,
      label: option,
      variant: current === option ? "secondary" : "ghost",
      onPress: IntentRef(
        "Gallery.ControlValueChanged",
        StaticPayload({
          controlId: control.id,
          value: option
        })
      ),
      style: {
        borderColor: current === option ? "accent" : "border",
        borderWidth: 1,
        borderRadius: "md",
        padding: "2"
      }
    })
  )

const controlEditor = (state: GalleryState, story: Story, control: StoryControl): View => {
  const current = controlValue(state, story, control)
  const baseLabel = Text({
    key: `${control.id}-label`,
    content: control.label,
    variant: "caption",
    color: "textMuted"
  })

  switch (control.kind) {
    case "boolean":
      return Stack({ key: `control-${control.id}`, direction: "row", gap: "2", align: "center" }, [
        baseLabel,
        Spacer({ key: `${control.id}-spacer`, flex: true }),
        Button({
          key: `${control.id}-toggle`,
          label: current === true ? "On" : "Off",
          variant: current === true ? "secondary" : "ghost",
          onPress: IntentRef(
            "Gallery.ControlValueChanged",
            StaticPayload({
              controlId: control.id,
              value: current !== true
            })
          ),
          style: { borderRadius: "md", padding: "2" }
        })
      ])
    case "text":
    case "number": {
      const fallback = control.value
      const nextValue: JsonPayload =
        control.kind === "number"
          ? typeof current === "number" && current !== 220
            ? 220
            : fallback
          : typeof current === "string" && current !== `${control.value} updated`
            ? `${control.value} updated`
            : fallback
      return Stack({ key: `control-${control.id}`, direction: "row", gap: "2", align: "center" }, [
        baseLabel,
        Spacer({ key: `${control.id}-spacer`, flex: true }),
        Button({
          key: `${control.id}-edit`,
          label: String(current ?? ""),
          variant: "ghost",
          onPress: IntentRef(
            "Gallery.ControlValueChanged",
            StaticPayload({
              controlId: control.id,
              value: nextValue
            })
          ),
          style: { borderRadius: "md", padding: "2", textAlign: "left" }
        })
      ])
    }
    case "enum":
    case "token":
      return Stack({ key: `control-${control.id}`, direction: "column", gap: "2" }, [
        baseLabel,
        Stack(
          { key: `${control.id}-options`, direction: "row", gap: "1", style: { flex: 1 } },
          optionButtons(story, control, current)
        )
      ])
  }
}

const componentNavigation = (state: GalleryState): View =>
  Card(
    {
      key: "component-nav",
      padding: "3",
      radius: "lg",
      style: { backgroundColor: "surface", borderColor: "border", borderWidth: 1, width: 240 }
    },
    [
      Stack({ key: "component-nav-stack", direction: "column", gap: "2" }, [
        Text({ key: "component-nav-title", content: "Components", variant: "title", color: "textPrimary" }),
        Button({
          key: "component-nav-docs",
          label: "Docs & foundations",
          variant: "ghost",
          onPress: IntentRef("Gallery.PageSelected", StaticPayload(foundationPageIds[0])),
          style: { borderColor: "border", borderWidth: 1, borderRadius: "md", padding: "2", textAlign: "left" }
        }),
        List(
          { key: "component-list" },
          state.storybook.groups.map((group) =>
            keyed(
              Button({
                key: `component-${group.component}`,
                label: `${group.component} (${group.stories.length})`,
                variant: state.activeComponent === group.component ? "secondary" : "ghost",
                onPress: IntentRef("Gallery.ComponentSelected", StaticPayload(group.component)),
                style: { borderRadius: "md", padding: "2", textAlign: "left" }
              })
            )
          )
        )
      ])
    ]
  )

const storyNavigation = (state: GalleryState): View => {
  const group = selectedStoryGroup(state)
  return Card(
    {
      key: "story-nav",
      padding: "3",
      radius: "lg",
      style: { backgroundColor: "surface", borderColor: "border", borderWidth: 1, width: 280 }
    },
    [
      Stack({ key: "story-nav-stack", direction: "column", gap: "2" }, [
        Text({ key: "story-nav-title", content: group.title, variant: "title", color: "textPrimary" }),
        Button({
          key: "story-nav-docs-page",
          label: `Open ${group.component} docs page`,
          variant: "ghost",
          onPress: IntentRef("Gallery.PageSelected", StaticPayload(componentPageId(group.component))),
          style: { borderColor: "border", borderWidth: 1, borderRadius: "md", padding: "2", textAlign: "left" }
        }),
        List(
          { key: "story-list" },
          group.stories.map((storyItem) =>
            keyed(
              Button({
                key: `story-${storyItem.id}`,
                label: `${storyItem.title}\n${storyItem.kind}`,
                variant: state.activeStoryId === storyItem.id ? "secondary" : "ghost",
                onPress: IntentRef("Gallery.StorySelected", StaticPayload(storyItem.id)),
                style: { borderRadius: "md", padding: "2", textAlign: "left" }
              })
            )
          )
        )
      ])
    ]
  )
}

const pagesNavigation = (state: GalleryState): View =>
  Card(
    {
      key: "pages-nav",
      padding: "3",
      radius: "lg",
      style: { backgroundColor: "surface", borderColor: "border", borderWidth: 1, width: 240 }
    },
    [
      Stack({ key: "pages-nav-stack", direction: "column", gap: "2" }, [
        Text({ key: "pages-nav-title", content: "Docs", variant: "title", color: "textPrimary" }),
        Button({
          key: "pages-nav-browser",
          label: "Back to story browser",
          variant: "ghost",
          onPress: IntentRef("Gallery.PageSelected", StaticPayload("")),
          style: { borderColor: "border", borderWidth: 1, borderRadius: "md", padding: "2", textAlign: "left" }
        }),
        Text({ key: "pages-nav-foundations", content: "Foundations", variant: "caption", color: "textMuted" }),
        ...foundationPages.map((page) =>
          Button({
            key: `pages-nav-${page.id}`,
            label: page.title,
            variant: state.activePageId === page.id ? "secondary" : "ghost",
            onPress: IntentRef("Gallery.PageSelected", StaticPayload(page.id)),
            style: { borderRadius: "md", padding: "2", textAlign: "left" }
          })
        ),
        Text({ key: "pages-nav-components", content: "Components", variant: "caption", color: "textMuted" }),
        List(
          { key: "pages-nav-component-list" },
          componentPages.map((page) =>
            keyed(
              Button({
                key: `pages-nav-${page.id}`,
                label: page.title,
                variant: state.activePageId === page.id ? "secondary" : "ghost",
                onPress: IntentRef("Gallery.PageSelected", StaticPayload(page.id)),
                style: { borderRadius: "md", padding: "2", textAlign: "left" }
              })
            )
          )
        )
      ])
    ]
  )

const pageMain = (state: GalleryState, page: GalleryPage): View =>
  Stack({ key: "gallery-main", direction: "column", gap: "3", style: { flex: 1, minWidth: 0 } }, [
    Stack({ key: "gallery-head", direction: "column", gap: "2" }, [
      Text({
        key: "gallery-title",
        content: "Effect Native component gallery",
        variant: "heading",
        color: "textPrimary"
      }),
      Text({
        key: "gallery-subtitle",
        content: page.description,
        variant: "body",
        color: "textMuted"
      }),
      toolbar(state)
    ]),
    Card(
      {
        key: "page-canvas",
        padding: "4",
        radius: "lg",
        style: { backgroundColor: "background", borderColor: "border", borderWidth: 1, minHeight: 260 }
      },
      [page.view]
    )
  ])

const toolbar = (state: GalleryState): View =>
  Stack({ key: "gallery-toolbar", direction: "row", gap: "2", align: "center" }, [
    Text({ key: "theme-label", content: "Theme", variant: "caption", color: "textMuted" }),
    ...galleryThemes.map((theme) =>
      Button({
        key: `theme-${theme.id}`,
        label: theme.label,
        variant: state.activeThemeId === theme.id ? "secondary" : "ghost",
        onPress: IntentRef("Gallery.ThemeSelected", StaticPayload(theme.id)),
        style: { borderRadius: "md", padding: "2" }
      })
    ),
    Spacer({ key: "toolbar-mid", size: "3" }),
    Text({ key: "viewport-label", content: "Viewport", variant: "caption", color: "textMuted" }),
    ...galleryViewports.map((viewport) =>
      Button({
        key: `viewport-${viewport.id}`,
        label: viewport.label,
        variant: state.activeViewportId === viewport.id ? "secondary" : "ghost",
        onPress: IntentRef("Gallery.ViewportSelected", StaticPayload(viewport.id)),
        style: { borderRadius: "md", padding: "2" }
      })
    )
  ])

const storyPreview = (state: GalleryState): View => {
  const storyItem = activeStory(state)
  return Card(
    {
      key: "component-preview",
      padding: "4",
      radius: "lg",
      style: {
        backgroundColor: "background",
        borderColor: "border",
        borderWidth: 1,
        minHeight: 260
      }
    },
    [
      Stack({ key: "component-preview-stack", direction: "column", gap: "3" }, [
        Stack({ key: "preview-head", direction: "row", align: "center", gap: "2" }, [
          Text({ key: "preview-title", content: storyItem.title, variant: "title", color: "textPrimary" }),
          Spacer({ key: "preview-head-spacer", flex: true }),
          Text({
            key: "preview-meta",
            content: `${storyItem.component} - ${activeViewport(state).label}`,
            variant: "caption",
            color: "textMuted"
          })
        ]),
        Text({ key: "preview-description", content: storyItem.description, variant: "body", color: "textMuted" }),
        storyItem.view
      ])
    ]
  )
}

const controlPanel = (state: GalleryState): View => {
  const storyItem = activeStory(state)
  return Card(
    {
      key: "control-panel",
      padding: "3",
      radius: "lg",
      style: { backgroundColor: "surface", borderColor: "border", borderWidth: 1 }
    },
    [
      Stack({ key: "control-panel-stack", direction: "column", gap: "3" }, [
        Text({ key: "control-panel-title", content: "Controls", variant: "title", color: "textPrimary" }),
        ...(storyItem.controls.length === 0
          ? [
              Text({
                key: "control-empty",
                content: "This story has no editable controls.",
                variant: "body",
                color: "textMuted"
              })
            ]
          : storyItem.controls.map((control) => controlEditor(state, storyItem, control)))
      ])
    ]
  )
}

const serializedPanel = (state: GalleryState): View => {
  const storyItem = activeStory(state)
  return Card(
    {
      key: "serialized-panel",
      padding: "3",
      radius: "lg",
      style: { backgroundColor: "surface", borderColor: "border", borderWidth: 1 }
    },
    [
      Stack({ key: "serialized-panel-stack", direction: "column", gap: "2" }, [
        Text({ key: "serialized-panel-title", content: "Serialized story", variant: "title", color: "textPrimary" }),
        TextField({
          key: "serialized-story",
          label: "JSON",
          value: serializeStory({
            ...storyItem,
            theme: activeTheme(state).theme,
            viewport: activeViewport(state).viewport
          }),
          multiline: true,
          onChange: IntentRef("Gallery.SerializedStoryChanged", ComponentValueBinding()),
          style: {
            minHeight: 220,
            borderColor: "border",
            borderWidth: 1,
            borderRadius: "md",
            padding: "3",
            color: "textPrimary",
            backgroundColor: "background"
          }
        })
      ])
    ]
  )
}

const galleryRootStyle = {
  minHeight: "full",
  backgroundColor: "background",
  variants: {
    platform: {
      ios: { paddingTop: "16" }
    }
  }
} as const

export const galleryView = (state: GalleryState): View => {
  const page = state.activePageId === "" ? undefined : galleryPageById(state.activePageId)
  if (page !== undefined) {
    return Stack(
      {
        key: "gallery-root",
        direction: { base: "column", lg: "row" },
        gap: "3",
        padding: "3",
        style: galleryRootStyle
      },
      [pagesNavigation(state), pageMain(state, page)]
    )
  }
  return Stack(
    {
      key: "gallery-root",
      direction: { base: "column", lg: "row" },
      gap: "3",
      padding: "3",
      style: galleryRootStyle
    },
    [
      componentNavigation(state),
      storyNavigation(state),
      Stack({ key: "gallery-main", direction: "column", gap: "3", style: { flex: 1, minWidth: 0 } }, [
        Stack({ key: "gallery-head", direction: "column", gap: "2" }, [
          Text({
            key: "gallery-title",
            content: "Effect Native component gallery",
            variant: "heading",
            color: "textPrimary"
          }),
          Text({
            key: "gallery-subtitle",
            content: "Stories are typed data shared by web, mobile, headless tests, and visual baselines.",
            variant: "body",
            color: "textMuted"
          }),
          toolbar(state)
        ]),
        storyPreview(state),
        Stack({ key: "gallery-lower", direction: { base: "column", lg: "row" }, gap: "3" }, [
          Stack({ key: "gallery-controls-column", direction: "column", gap: "3", style: { flex: 1 } }, [
            controlPanel(state)
          ]),
          Stack({ key: "gallery-json-column", direction: "column", gap: "3", style: { flex: 1 } }, [
            serializedPanel(state)
          ])
        ])
      ])
    ]
  )
}

export interface GalleryRuntime {
  readonly state: SubscriptionRef.SubscriptionRef<GalleryState>
  readonly program: ViewProgram<GalleryState>
  readonly registry: IntentRegistry
  readonly report: IntentReporter
}

export const makeGalleryRuntime = (storybook: Storybook = defaultStorybook): Effect.Effect<GalleryRuntime> =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make(initialGalleryState(storybook))
    const program = makeViewProgramFromState(state, galleryView, {
      redactState: (current) => Schema.encodeSync(GalleryStateSchema)(current) as unknown as JsonPayload
    })
    const handlers: IntentHandlers<typeof galleryIntentDefinitions> = {
      "Gallery.StorySelected": (storyId) =>
        SubscriptionRef.update(state, (current) => {
          const nextStory = storyById(storyId, current.storybook)
          return nextStory === undefined
            ? current
            : {
                ...current,
                activeComponent: nextStory.component,
                activeStoryId: nextStory.id
              }
        }),
      "Gallery.ComponentSelected": (component) =>
        SubscriptionRef.update(state, (current) => {
          const firstStory = storiesForComponent(component, current.storybook)[0]
          return firstStory === undefined
            ? current
            : {
                ...current,
                activeComponent: component,
                activeStoryId: firstStory.id
              }
        }),
      "Gallery.ControlValueChanged": (payload) =>
        SubscriptionRef.update(state, (current) => ({
          ...current,
          controlValues: {
            ...current.controlValues,
            [current.activeStoryId]: {
              ...(current.controlValues[current.activeStoryId] ?? {}),
              [payload.controlId]: payload.value
            }
          }
        })),
      "Gallery.ThemeSelected": (themeId) =>
        SubscriptionRef.update(state, (current) => ({ ...current, activeThemeId: themeId })),
      "Gallery.ViewportSelected": (viewportId) =>
        SubscriptionRef.update(state, (current) => ({ ...current, activeViewportId: viewportId })),
      "Gallery.SerializedStoryChanged": () => Effect.void,
      "Gallery.PageSelected": (pageId) =>
        SubscriptionRef.update(state, (current) =>
          pageId === "" || galleryPageById(pageId) !== undefined ? { ...current, activePageId: pageId } : current
        )
    }
    const registry = yield* makeIntentRegistry(galleryIntentDefinitions, handlers, { now: () => 0 })
    const report: IntentReporter = (ref, runtimeValue) => registry.dispatch(resolveIntentRef(ref, runtimeValue))

    return { state, program, registry, report }
  })

export const makeStoryIntentRegistry = (): Effect.Effect<IntentRegistry> =>
  makeIntentRegistry(
    storyIntentDefinitions,
    {
      "GalleryStory.Pressed": () => Effect.void,
      "GalleryStory.Changed": () => Effect.void,
      "GalleryStory.Submitted": () => Effect.void,
      "GalleryStory.Dismissed": () => Effect.void
    },
    { now: () => 0 }
  )

export const replayStoryInteractions = (
  input: Story
): Effect.Effect<ReadonlyArray<string>, import("@effect-native/core").IntentError> =>
  Effect.gen(function* () {
    const registry = yield* makeStoryIntentRegistry()
    for (const interaction of input.interactions) {
      yield* registry.dispatch(interaction.intent)
    }
    const events = yield* registry.events
    return events.map((event) => event.intent.name)
  })
