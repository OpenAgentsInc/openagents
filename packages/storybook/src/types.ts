/**
 * @since 1.0.0
 */

import type {
  StorybookConfig as BaseStorybookConfig,
  TypescriptOptions as BaseTypescriptOptions,
  WebRenderer
} from "@storybook/types"
import type * as Fx from "@typed/fx/Fx"
import type { RenderEvent } from "@typed/template"

/**
 * Custom renderer interface for Typed components in Storybook
 *
 * @since 1.0.0
 * @category Renderer
 */
export interface TypedRenderer extends WebRenderer {
  component: TypedComponent<any>
  storyResult: Fx.Fx<RenderEvent, any, any>
}

/**
 * Function signature for Typed components
 *
 * @since 1.0.0
 * @category Component
 */
export interface TypedComponent<Args = Record<string, unknown>> {
  (args: Args, ...children: ReadonlyArray<Fx.Fx<RenderEvent, any, any>>): Fx.Fx<RenderEvent, any, any>
}

/**
 * Function signature for HTML components
 *
 * @since 1.0.0
 * @category Component
 */
export interface HtmlComponent<Args = Record<string, unknown>> {
  (args: Args): HTMLElement
}

/**
 * Union type for components that can be either HTML or Typed
 *
 * @since 1.0.0
 * @category Component
 */
export type Component<Args = Record<string, unknown>> = TypedComponent<Args> | HtmlComponent<Args>

/**
 * Story metadata with Typed component integration
 *
 * @since 1.0.0
 * @category Story
 */
export interface Meta<Args = Record<string, unknown>> {
  component?: Component<Args>
  title?: string
  parameters?: Record<string, any>
  argTypes?: Record<keyof Args, any>
  args?: Partial<Args>
  decorators?: ReadonlyArray<(story: () => any, context: any) => any>
  render?: (args: Args) => Fx.Fx<RenderEvent, any, any> | HTMLElement
}

/**
 * Individual story configuration
 *
 * @since 1.0.0
 * @category Story
 */
export interface StoryObj<Args = Record<string, unknown>> {
  args?: Partial<Args>
  argTypes?: Record<keyof Args, any>
  parameters?: Record<string, any>
  render?: (args: Args) => Fx.Fx<RenderEvent, any, any> | HTMLElement
  play?: (context: any) => Promise<void> | void
}

/**
 * Storybook configuration with Vite integration
 *
 * @since 1.0.0
 * @category Configuration
 */
export interface StorybookConfig extends BaseStorybookConfig {
  core?: BaseStorybookConfig["core"]
  viteFinal?: (config: any, options: { configType: "DEVELOPMENT" | "PRODUCTION" }) => any | Promise<any>
  typescript?: BaseTypescriptOptions
}

/**
 * Framework-specific options
 *
 * @since 1.0.0
 * @category Configuration
 */
export interface FrameworkOptions {
  builder?: Record<string, any>
}
