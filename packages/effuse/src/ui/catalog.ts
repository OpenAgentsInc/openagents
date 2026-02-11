import { Effect, Schema } from "effect"
import type { UIElement, UITree, ValidationMode } from "./types.js"
import { isVisibilityCondition } from "./visibility.js"
import type { ValidationFunction } from "./validation.js"

export interface ComponentDefinition<P = Record<string, unknown>> {
  props: Schema.Schema<P>
  hasChildren?: boolean
  description?: string
}

export interface ActionDefinition<P = Record<string, unknown>> {
  params?: Schema.Schema<P>
  description?: string
}

export interface CatalogConfig<
  TComponents extends Record<string, ComponentDefinition>,
  TActions extends Record<string, ActionDefinition>,
  TFunctions extends Record<string, ValidationFunction>
> {
  name?: string
  components: TComponents
  actions?: TActions
  functions?: TFunctions
  validation?: ValidationMode
}

export interface Catalog<
  TComponents extends Record<string, ComponentDefinition>,
  TActions extends Record<string, ActionDefinition>,
  TFunctions extends Record<string, ValidationFunction>
> {
  readonly name: string
  readonly componentNames: (keyof TComponents)[]
  readonly actionNames: (keyof TActions)[]
  readonly functionNames: (keyof TFunctions)[]
  readonly validation: ValidationMode
  readonly components: TComponents
  readonly actions: TActions
  readonly functions: TFunctions
  hasComponent(type: string): boolean
  hasAction(name: string): boolean
  hasFunction(name: string): boolean
  validateElement(element: unknown): {
    success: boolean
    data?: UIElement
    errors?: string[]
  }
  validateTree(tree: unknown): {
    success: boolean
    data?: UITree
    errors?: string[]
  }
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null

const isNonEmptyString = (value: unknown): value is string =>
  typeof value === "string" && value.length > 0

const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((c): c is string => typeof c === "string")

const decodeSchema = <A>(schema: Schema.Schema<A>, value: unknown) =>
  Effect.runSync(Effect.either(Schema.decodeUnknown(schema)(value)))

export const createCatalog = <
  TComponents extends Record<string, ComponentDefinition>,
  TActions extends Record<string, ActionDefinition> = Record<string, ActionDefinition>,
  TFunctions extends Record<string, ValidationFunction> = Record<string, ValidationFunction>
>(
  config: CatalogConfig<TComponents, TActions, TFunctions>
): Catalog<TComponents, TActions, TFunctions> => {
  const {
    name = "unnamed",
    components,
    actions = {} as TActions,
    functions = {} as TFunctions,
    validation = "strict",
  } = config

  const componentNames = Object.keys(components) as (keyof TComponents)[]
  const actionNames = Object.keys(actions) as (keyof TActions)[]
  const functionNames = Object.keys(functions) as (keyof TFunctions)[]

  const warn = (message: string) => {
    if (validation === "warn") {
      Effect.runSync(Effect.logWarning(`[Effuse/Catalog] ${message}`))
    }
  }

  const validateElement = (element: unknown) => {
    if (!isRecord(element)) {
      return { success: false, errors: ["Element must be an object"] }
    }

    const key = element.key
    const type = element.type
    const props = element.props
    const children = element.children
    const visible = element.visible

    const errors: string[] = []

    if (typeof key !== "string" || !key) {
      errors.push("Element.key must be a non-empty string")
    }

    if (typeof type !== "string" || !type) {
      errors.push("Element.type must be a non-empty string")
    }

    if (children !== undefined && !Array.isArray(children)) {
      errors.push("Element.children must be an array of strings when provided")
    }

    if (Array.isArray(children) && children.some((child: unknown) => typeof child !== "string")) {
      errors.push("Element.children entries must be strings")
    }

    if (visible !== undefined && !isVisibilityCondition(visible)) {
      errors.push("Element.visible must be a valid visibility condition")
    }

    const component = typeof type === "string" ? components[type as keyof TComponents] : undefined
    if (!component) {
      const message = `Unknown component type: ${String(type)}`
      if (validation === "strict") {
        errors.push(message)
      } else {
        warn(message)
      }
    }

    let finalProps: Record<string, unknown> =
      typeof element.props === "object" && element.props !== null
        ? (element.props as Record<string, unknown>)
        : {}

    if (component && props !== undefined) {
      const decoded = decodeSchema(component.props, props)
      if (decoded._tag === "Left") {
        const message = `Invalid props for ${String(type)}: ${decoded.left}`
        if (validation === "strict") {
          errors.push(message)
        } else {
          warn(message)
        }
      } else {
        finalProps = decoded.right as Record<string, unknown>
      }
    }

    if (errors.length > 0) {
      return { success: false, errors }
    }

    if (!isNonEmptyString(key) || !isNonEmptyString(type)) {
      return { success: false, errors: ["Element.key and type must be non-empty strings"] }
    }

    const data: UIElement = {
      key,
      type,
      props: finalProps,
      ...(isStringArray(children) ? { children } : {}),
      ...(visible !== undefined && isVisibilityCondition(visible) ? { visible } : {}),
    }
    return { success: true, data }
  }

  const validateTree = (tree: unknown) => {
    if (!isRecord(tree)) {
      return { success: false, errors: ["Tree must be an object"] }
    }

    const root = tree.root
    const elements = tree.elements
    const errors: string[] = []

    if (typeof root !== "string") {
      errors.push("Tree.root must be a string")
    }

    if (!isRecord(elements)) {
      errors.push("Tree.elements must be an object")
    }

    if (errors.length > 0) {
      return { success: false, errors }
    }

    const elementsRecord = elements as Record<string, unknown>
    const validatedElements: Record<string, UIElement> = {}
    for (const [key, element] of Object.entries(elementsRecord)) {
      const result = validateElement(element)
      if (!result.success) {
        errors.push(...(result.errors ?? []).map((err) => `[${key}] ${err}`))
      } else if (result.data) {
        validatedElements[key] = result.data
      }
    }

    if (errors.length > 0) {
      return { success: false, errors }
    }

    const validatedTree: UITree = {
      root: root as string,
      elements: validatedElements,
    }

    if (validatedTree.root && !validatedTree.elements[validatedTree.root]) {
      const message = `Tree.root "${validatedTree.root}" missing from elements`
      if (validation === "strict") {
        return { success: false, errors: [message] }
      }
      warn(message)
    }

    return { success: true, data: validatedTree }
  }

  return {
    name,
    componentNames,
    actionNames,
    functionNames,
    validation,
    components,
    actions,
    functions,
    hasComponent(type: string) {
      return type in components
    },
    hasAction(name: string) {
      return name in actions
    },
    hasFunction(name: string) {
      return name in functions
    },
    validateElement,
    validateTree,
  }
}

export const generateCatalogPrompt = <
  TComponents extends Record<string, ComponentDefinition>,
  TActions extends Record<string, ActionDefinition>,
  TFunctions extends Record<string, ValidationFunction>
>(
  catalog: Catalog<TComponents, TActions, TFunctions>
): string => {
  const lines: string[] = [
    `# ${catalog.name} Component Catalog`,
    "",
    "## Available Components",
    "",
  ]

  for (const name of catalog.componentNames) {
    const def = catalog.components[name]!
    lines.push(`### ${String(name)}`)
    if (def.description) {
      lines.push(def.description)
    }
    lines.push("")
  }

  if (catalog.actionNames.length > 0) {
    lines.push("## Available Actions")
    lines.push("")
    for (const name of catalog.actionNames) {
      const def = catalog.actions[name]!
      lines.push(`- \`${String(name)}\`${def.description ? `: ${def.description}` : ""}`)
    }
    lines.push("")
  }

  lines.push("## Visibility Conditions")
  lines.push("")
  lines.push("Components can have a `visible` property:")
  lines.push("- `true` / `false` - Always visible/hidden")
  lines.push('- `{ \"path\": \"/data/path\" }` - Visible when path is truthy')
  lines.push('- `{ \"auth\": \"signedIn\" }` - Visible when user is signed in')
  lines.push('- `{ \"and\": [...] }` - All conditions must be true')
  lines.push('- `{ \"or\": [...] }` - Any condition must be true')
  lines.push('- `{ \"not\": {...} }` - Negates a condition')
  lines.push('- `{ \"eq\": [a, b] }` - Equality check')
  lines.push("")

  lines.push("## Validation Functions")
  lines.push("")
  lines.push(
    "Built-in: `required`, `email`, `minLength`, `maxLength`, `pattern`, `min`, `max`, `url`"
  )
  if (catalog.functionNames.length > 0) {
    lines.push(`Custom: ${catalog.functionNames.map(String).join(", ")}`)
  }
  lines.push("")

  return lines.join("\n")
}
