/**
 * @since 1.0.0
 */

import type {
  StorybookConfig as BaseStorybookConfig,
  TypescriptOptions as BaseTypescriptOptions,
  WebRenderer
} from "@storybook/types"
import type { RenderEvent } from "@typed/template"
import type * as Fx from "@typed/fx/Fx"

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
export interface TypedComponent<Args = {}> {
  (args: Args, ...children: ReadonlyArray<Fx.Fx<RenderEvent, any, any>>): Fx.Fx<RenderEvent, any, any>
}

/**
 * Story metadata with Typed component integration
 * 
 * @since 1.0.0
 * @category Story
 */
export interface Meta<Args = {}> {
  component?: TypedComponent<Args>
  title?: string
  parameters?: Record<string, any>
  argTypes?: Record<keyof Args, any>
  args?: Partial<Args>
  decorators?: ReadonlyArray<(story: () => any, context: any) => any>
  render?: (args: Args) => Fx.Fx<RenderEvent, any, any>
}

/**
 * Individual story configuration
 * 
 * @since 1.0.0
 * @category Story
 */
export interface StoryObj<Args = {}, T = Meta<Args>> {
  args?: Partial<Args>
  argTypes?: Record<keyof Args, any>
  parameters?: Record<string, any>
  render?: (args: Args) => Fx.Fx<RenderEvent, any, any>
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