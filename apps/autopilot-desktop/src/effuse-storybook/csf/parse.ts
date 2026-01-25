/**
 * CSF Module Parsing Logic
 */

import type { CsfFile, CsfStory, Meta, StoryObj } from "./csf"
import { inferTitle, storyNameFromExport, toId } from "./id"

/**
 * Parses a loaded ES module into a structured CsfFile
 */
export function parseCsfModule(filePath: string, mod: any): CsfFile {
  const meta: Meta = mod.default

  if (!meta) {
    throw new Error(`Story file ${filePath} missing default export (Meta)`)
  }

  const title = meta.title || inferTitle(filePath)
  const stories: CsfStory[] = []

  const exportKeys = Object.keys(mod).filter((k) => k !== "default")

  for (const key of exportKeys) {
    // 1. Check include/exclude
    if (!isExportIncluded(key, meta.includeStories, meta.excludeStories)) {
      continue
    }

    const exportVal = mod[key]

    // 2. We only support object-style stories for now (CSF 3)
    // Legacy function stories could be supported here later
    if (typeof exportVal !== "object" || exportVal === null) {
      continue
    }

    const storyObj = exportVal as StoryObj
    const name = storyObj.name || storyNameFromExport(key)
    const id = toId(title, key)

    // 3. Merge Args (Meta args + Story args)
    const args = { ...meta.args, ...storyObj.args }

    // 4. Merge Parameters (Meta + Story)
    // Note: A real deep merge is better, but shallow merge for v1
    const parameters = { ...meta.parameters, ...storyObj.parameters }

    // 5. Merge Decorators (Meta + Story)
    const decorators = [
      ...(meta.decorators || []),
      ...(storyObj.decorators || []),
    ]

    // 6. Normalize Render
    // If story.render missing, we'll assume a default render using meta.component
    // This logic is deferred to the runtime renderer usually, but we can set up the function here.
    // However, the types say render is optional. We keep it optional in CsfStory?
    // Let's normalize it to a function here if possible, or keep as is.
    // To satisfy CsfStory type which requires render:
    const render = storyObj.render || createDefaultRender(meta.component)

    stories.push({
      id,
      name,
      title,
      importPath: filePath,
      args,
      parameters,
      decorators,
      render,
      play: storyObj.play,
      tags: [...(meta.tags || []), ...(storyObj.tags || [])],
    })
  }

  return {
    filePath,
    meta,
    stories,
  }
}

/**
 * Helper to determine if an export should be included as a story
 */
function isExportIncluded(
  key: string,
  include?: (string | RegExp)[] | RegExp,
  exclude?: (string | RegExp)[] | RegExp
): boolean {
  // If include is specified, must match
  if (include) {
    const isIncluded = matchPattern(key, include)
    if (!isIncluded) return false
  }

  // If exclude is specified, must NOT match
  if (exclude) {
    const isExcluded = matchPattern(key, exclude)
    if (isExcluded) return false
  }

  return true
}

function matchPattern(
  key: string,
  pattern: (string | RegExp)[] | RegExp
): boolean {
  if (Array.isArray(pattern)) {
    return pattern.some((p) =>
      typeof p === "string" ? key === p : p.test(key)
    )
  }
  return pattern.test(key)
}

/**
 * Creates a default render function if none provided.
 * This expects meta.component to be defined.
 */
function createDefaultRender(component: any): any {
  if (!component) {
    return () => "<div>Error: No component or render function defined</div>"
  }

  // If component is a function (Effuse component convention), call it
  // If it's an object with render method, call that
  return (args: any, ctx: any) => {
    if (typeof component === "function") {
      return component(args, ctx)
    }
    if (component && typeof component.render === "function") {
      return component.render(args, ctx)
    }
    return `<div>Unknown component type for ${ctx.title}</div>`
  }
}
