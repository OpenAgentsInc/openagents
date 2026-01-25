/**
 * CSF (Component Story Format) Type Definitions for Effuse
 * Based on CSF 3.0
 */

// Context provided to the story rendering
export interface StoryContext<Args = any> {
  id: string
  name: string
  title: string
  args: Args
  argTypes?: Record<string, any>
  parameters: Record<string, any>
  viewMode: "story" | "docs"
}

// Context provided to the play function
export interface PlayContext<Args = any> extends StoryContext<Args> {
  canvasElement: HTMLElement
  // canvas: CanvasQueries  // To be implemented in Task F
  // userEvent: UserEvent   // To be implemented in Task F
  // expect: Expect         // To be implemented in Task F
}

// Render output format for Effuse
export type StoryRenderOutput =
  | string
  | { html: string; swapMode?: "inner" | "outer" | "morph" }
  | Promise<string>
  | Promise<{ html: string; swapMode?: "inner" | "outer" | "morph" }>

// A function that decorates a story
export type Decorator<Args = any> = (
  story: StoryFn<Args>,
  ctx: StoryContext<Args>
) => StoryRenderOutput

// The function that renders a story
export type StoryFn<Args = any> = (
  args: Args,
  ctx: StoryContext<Args>
) => StoryRenderOutput

// CSF 3.0 Meta (Default Export)
export interface Meta<C = any, Args = any> {
  title?: string
  component?: C // Optional in loose usage, but typically required
  decorators?: Decorator<Args>[]
  parameters?: Record<string, any>
  args?: Partial<Args> // Default args for all stories
  argTypes?: Record<string, any>
  includeStories?: (string | RegExp)[] | RegExp
  excludeStories?: (string | RegExp)[] | RegExp
  tags?: string[]
}

// CSF 3.0 Story Object (Named Export)
export interface StoryObj<Args = any> {
  name?: string // Display name override
  args?: Partial<Args> // Story-specific args
  parameters?: Record<string, any>
  decorators?: Decorator<Args>[]
  render?: StoryFn<Args> // Custom render function
  play?: ((ctx: PlayContext<Args>) => Promise<void> | void) | undefined
  tags?: string[]
}

// Internal representation of a normalized story
export interface CsfStory<Args = any> {
  id: string
  name: string
  title: string
  importPath: string
  args: Args
  parameters: Record<string, any>
  decorators: Decorator<Args>[]
  render: StoryFn<Args>
  play?: ((ctx: PlayContext<Args>) => Promise<void> | void) | undefined
  tags: string[]
}

// Represents a parsed CSF module file
export interface CsfFile {
  filePath: string
  meta: Meta
  stories: CsfStory[]
}
