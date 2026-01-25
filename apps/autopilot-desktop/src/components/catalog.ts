import { Schema } from "effect"
import type { ComponentRegistry } from "../effuse/ui/index.js"
import { createCatalog } from "../effuse/ui/index.js"
import {
  Alert,
  Button,
  Canvas,
  CodeBlock,
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  Diff,
  Edge,
  Heading,
  Input,
  Message,
  Node,
  NodeAction,
  NodeContent,
  NodeDescription,
  NodeFooter,
  NodeHeader,
  NodeTitle,
  Panel,
  Reasoning,
  Row,
  Select,
  Stack,
  Text,
  TextArea,
  Toggle,
  ToolCall,
} from "./ai-elements/index.js"

const TextToneSchema = Schema.Literal("default", "muted", "accent", "danger")
const TextSizeSchema = Schema.Literal("xs", "sm", "md")
const HeadingLevelSchema = Schema.Literal(1, 2, 3, 4)
const NodeToneSchema = Schema.Literal("default", "accent", "muted")
const AlertToneSchema = Schema.Literal("info", "warn", "error", "success")
const ButtonVariantSchema = Schema.Literal("primary", "secondary", "danger")
const ButtonSizeSchema = Schema.Literal("sm", "md", "lg")
const InputTypeSchema = Schema.Literal("text", "number", "password", "search")
const DynamicPathSchema = Schema.Struct({ path: Schema.String })
const DynamicStringSchema = Schema.Union(Schema.String, DynamicPathSchema)
const DynamicNumberSchema = Schema.Union(Schema.Number, DynamicPathSchema)
const DynamicBooleanSchema = Schema.Union(Schema.Boolean, DynamicPathSchema)

export const effuseCatalog = createCatalog({
  name: "Effuse Autopilot",
  validation: "strict",
  components: {
    canvas: {
      props: Schema.Struct({
        title: Schema.optional(DynamicStringSchema),
        subtitle: Schema.optional(DynamicStringSchema),
        status: Schema.optional(DynamicStringSchema),
      }),
      hasChildren: true,
      description: "Primary canvas container for Autopilot UI.",
    },
    stack: {
      props: Schema.Struct({
        gap: Schema.optional(Schema.Number),
        align: Schema.optional(Schema.Literal("start", "center", "end", "stretch")),
      }),
      hasChildren: true,
      description: "Vertical layout stack.",
    },
    row: {
      props: Schema.Struct({
        gap: Schema.optional(Schema.Number),
        align: Schema.optional(Schema.Literal("start", "center", "end", "stretch")),
        justify: Schema.optional(
          Schema.Literal("start", "center", "end", "between")
        ),
      }),
      hasChildren: true,
      description: "Horizontal layout row.",
    },
    panel: {
      props: Schema.Struct({
        title: Schema.optional(Schema.String),
        subtitle: Schema.optional(Schema.String),
      }),
      hasChildren: true,
      description: "Card-like panel container.",
    },
    node: {
      props: Schema.Struct({
        tone: Schema.optional(NodeToneSchema),
      }),
      hasChildren: true,
      description: "Canvas node container.",
    },
    node_header: {
      props: Schema.Struct({}),
      hasChildren: true,
      description: "Node header row.",
    },
    node_title: {
      props: Schema.Struct({
        text: DynamicStringSchema,
      }),
      description: "Node title text.",
    },
    node_description: {
      props: Schema.Struct({
        text: DynamicStringSchema,
      }),
      description: "Node description text.",
    },
    node_action: {
      props: Schema.Struct({}),
      hasChildren: true,
      description: "Node header action slot.",
    },
    node_content: {
      props: Schema.Struct({}),
      hasChildren: true,
      description: "Node content body.",
    },
    node_footer: {
      props: Schema.Struct({}),
      hasChildren: true,
      description: "Node footer row.",
    },
    edge: {
      props: Schema.Struct({
        path: Schema.String,
        dashed: Schema.optional(Schema.Boolean),
        animated: Schema.optional(Schema.Boolean),
        stroke: Schema.optional(Schema.String),
      }),
      description: "SVG path edge between nodes.",
    },
    text: {
      props: Schema.Struct({
        text: DynamicStringSchema,
        tone: Schema.optional(TextToneSchema),
        size: Schema.optional(TextSizeSchema),
      }),
      description: "Inline text.",
    },
    heading: {
      props: Schema.Struct({
        text: DynamicStringSchema,
        level: Schema.optional(HeadingLevelSchema),
      }),
      description: "Heading text with levels.",
    },
    code_block: {
      props: Schema.Struct({
        code: DynamicStringSchema,
        language: Schema.optional(DynamicStringSchema),
      }),
      description: "Code block with optional language.",
    },
    button: {
      props: Schema.Struct({
        label: DynamicStringSchema,
        disabled: Schema.optional(DynamicBooleanSchema),
        variant: Schema.optional(ButtonVariantSchema),
        size: Schema.optional(ButtonSizeSchema),
        action: Schema.optional(Schema.Unknown),
      }),
      description: "Action button.",
    },
    input: {
      props: Schema.Struct({
        name: Schema.String,
        label: Schema.optional(DynamicStringSchema),
        value: Schema.optional(DynamicStringSchema),
        placeholder: Schema.optional(DynamicStringSchema),
        type: Schema.optional(InputTypeSchema),
        disabled: Schema.optional(DynamicBooleanSchema),
        action: Schema.optional(Schema.Unknown),
        trigger: Schema.optional(Schema.Literal("change", "input")),
      }),
      description: "Text input field.",
    },
    textarea: {
      props: Schema.Struct({
        name: Schema.String,
        label: Schema.optional(DynamicStringSchema),
        value: Schema.optional(DynamicStringSchema),
        placeholder: Schema.optional(DynamicStringSchema),
        rows: Schema.optional(DynamicNumberSchema),
        disabled: Schema.optional(DynamicBooleanSchema),
        action: Schema.optional(Schema.Unknown),
        trigger: Schema.optional(Schema.Literal("change", "input")),
      }),
      description: "Multiline text area field.",
    },
    select: {
      props: Schema.Struct({
        name: Schema.String,
        label: Schema.optional(DynamicStringSchema),
        value: Schema.optional(DynamicStringSchema),
        disabled: Schema.optional(DynamicBooleanSchema),
        action: Schema.optional(Schema.Unknown),
        trigger: Schema.optional(Schema.Literal("change")),
        options: Schema.Array(
          Schema.Struct({
            label: DynamicStringSchema,
            value: DynamicStringSchema,
          })
        ),
      }),
      description: "Select dropdown field.",
    },
    toggle: {
      props: Schema.Struct({
        name: Schema.String,
        label: Schema.optional(DynamicStringSchema),
        checked: Schema.optional(DynamicBooleanSchema),
        disabled: Schema.optional(DynamicBooleanSchema),
        action: Schema.optional(Schema.Unknown),
        trigger: Schema.optional(Schema.Literal("change")),
      }),
      description: "Checkbox toggle.",
    },
    alert: {
      props: Schema.Struct({
        title: Schema.optional(DynamicStringSchema),
        message: DynamicStringSchema,
        tone: Schema.optional(AlertToneSchema),
      }),
      description: "Alert banner.",
    },
    conversation: {
      props: Schema.Struct({}),
      hasChildren: true,
      description: "Conversation container.",
    },
    conversation_content: {
      props: Schema.Struct({}),
      hasChildren: true,
      description: "Conversation content stack.",
    },
    conversation_empty_state: {
      props: Schema.Struct({
        title: Schema.optional(DynamicStringSchema),
        description: Schema.optional(DynamicStringSchema),
      }),
      description: "Conversation empty state placeholder.",
    },
    message: {
      props: Schema.Struct({
        role: DynamicStringSchema,
        text: DynamicStringSchema,
        isStreaming: Schema.optional(DynamicBooleanSchema),
      }),
      description: "Chat message bubble.",
    },
    reasoning: {
      props: Schema.Struct({
        summary: DynamicStringSchema,
        content: DynamicStringSchema,
        isStreaming: Schema.optional(DynamicBooleanSchema),
      }),
      description: "Reasoning summary block.",
    },
    tool_call: {
      props: Schema.Struct({
        title: DynamicStringSchema,
        detail: DynamicStringSchema,
        status: Schema.optional(DynamicStringSchema),
        output: Schema.optional(DynamicStringSchema),
        durationMs: Schema.optional(DynamicNumberSchema),
      }),
      description: "Tool call card.",
    },
    diff: {
      props: Schema.Struct({
        title: DynamicStringSchema,
        diff: DynamicStringSchema,
        status: Schema.optional(DynamicStringSchema),
      }),
      description: "Diff viewer block.",
    },
  },
  actions: {
    "ui.set": {
      params: Schema.Struct({
        path: Schema.String,
        value: Schema.Unknown,
      }),
      description: "Update the UI data model at a path.",
    },
    "ui.start": {
      params: Schema.Struct({
        workspacePath: Schema.String,
        prompt: Schema.optional(Schema.String),
      }),
      description: "Start an Autopilot session with an optional prompt.",
    },
    "ui.refresh": {
      description: "Refresh the UI tree or data model.",
    },
  },
})

export const componentRegistry: ComponentRegistry = {
  canvas: ({ props, children }) => Canvas({ ...props, children }),
  stack: ({ props, children }) => Stack({ ...props, children }),
  row: ({ props, children }) => Row({ ...props, children }),
  panel: ({ props, children }) => Panel({ ...props, children }),
  node: ({ props, children }) => Node({ ...props, children }),
  node_header: ({ children }) => NodeHeader({ children: children ?? [] }),
  node_title: ({ props }) => NodeTitle(props),
  node_description: ({ props }) => NodeDescription(props),
  node_action: ({ children }) => NodeAction({ children: children ?? [] }),
  node_content: ({ children }) => NodeContent({ children: children ?? [] }),
  node_footer: ({ children }) => NodeFooter({ children: children ?? [] }),
  edge: ({ props }) => Edge(props),
  text: ({ props }) => Text(props),
  heading: ({ props }) => Heading(props),
  code_block: ({ props }) => CodeBlock(props),
  button: ({ props }) => Button(props),
  input: ({ props }) => Input(props),
  textarea: ({ props }) => TextArea(props),
  select: ({ props }) => Select(props),
  toggle: ({ props }) => Toggle(props),
  alert: ({ props }) => Alert(props),
  conversation: ({ children }) => Conversation({ children: children ?? [] }),
  conversation_content: ({ children }) => ConversationContent({ children: children ?? [] }),
  conversation_empty_state: ({ props }) => ConversationEmptyState(props),
  message: ({ props }) => Message(props),
  reasoning: ({ props }) => Reasoning(props),
  tool_call: ({ props }) => ToolCall(props),
  diff: ({ props }) => Diff(props),
}
