import { html, joinTemplates } from "../template/html.js"
import { Effect } from "effect"
import type { TemplateResult } from "../template/types.js"
import type { AuthState, DataModel, UIElement, UITree } from "./types.js"
import { resolveDynamicObject } from "./data.js"
import { evaluateVisibility } from "./visibility.js"

export interface ComponentRenderProps<P = Record<string, unknown>> {
  element: UIElement
  props: P
  children?: TemplateResult[]
  dataModel: DataModel
  authState?: AuthState
}

export type ComponentRenderer<P = Record<string, unknown>> = (
  props: ComponentRenderProps<P>
) => TemplateResult

export type ComponentRegistry = Record<string, ComponentRenderer<any>>

export interface RenderOptions {
  dataModel: DataModel
  authState?: AuthState
  fallback?: ComponentRenderer
}

const emptyTemplate = html``

const renderElement = (
  tree: UITree,
  elementKey: string,
  registry: ComponentRegistry,
  options: RenderOptions
): TemplateResult => {
  const element = tree.elements[elementKey]
  if (!element) {
    return emptyTemplate
  }

  const isVisible = evaluateVisibility(element.visible, {
    dataModel: options.dataModel,
    ...(options.authState ? { authState: options.authState } : {}),
  })

  if (!isVisible) {
    return emptyTemplate
  }

  const renderer = registry[element.type] ?? options.fallback
  if (!renderer) {
    Effect.runSync(Effect.logWarning(`[Effuse/UI] No renderer for component type: ${element.type}`))
    return emptyTemplate
  }

  const resolvedProps = resolveDynamicObject(
    element.props,
    options.dataModel
  ) as Record<string, unknown>

  const children = element.children
    ?.map((childKey) => renderElement(tree, childKey, registry, options))
    .filter((child) => child.parts.length > 0)

  return renderer({
    element,
    props: resolvedProps,
    dataModel: options.dataModel,
    ...(children ? { children } : {}),
    ...(options.authState ? { authState: options.authState } : {}),
  })
}

export const renderTree = (
  tree: UITree | null,
  registry: ComponentRegistry,
  options: RenderOptions
): TemplateResult => {
  if (!tree || !tree.root) {
    return emptyTemplate
  }

  const rootElement = tree.elements[tree.root]
  if (!rootElement) {
    return emptyTemplate
  }

  const rootTemplate = renderElement(tree, tree.root, registry, options)
  if (rootTemplate.parts.length === 0) {
    return emptyTemplate
  }

  return joinTemplates([rootTemplate])
}
