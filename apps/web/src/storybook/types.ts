import type { TemplateResult } from "@openagentsinc/effuse"

/**
 * Effuse Storybook primitives (React-free).
 *
 * Stories are pure render functions returning `TemplateResult` so they can be:
 * - SSR-rendered by the Worker host
 * - client-navigated by Effuse Router (CSR)
 * - screenshot-tested deterministically
 */
export type StoryKind = "atom" | "molecule" | "organism"

export type Story = {
  /** Stable id used in URLs + visual snapshots. Must be URL-segment safe. */
  readonly id: string
  /** Display title. Use `/` to represent hierarchy. */
  readonly title: string
  readonly kind: StoryKind
  readonly render: () => TemplateResult
}

export type StoryMeta = Pick<Story, "id" | "title" | "kind">

