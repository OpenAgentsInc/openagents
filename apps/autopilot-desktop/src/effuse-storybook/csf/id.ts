/**
 * ID Generation and Title Inference
 */

import { startCase } from "./startCase"

/**
 * Infers the story title from the file path if not provided in Meta.
 *
 * Rules:
 * - Remove leading src/ or story root
 * - Remove extension (.stories.ts, etc)
 * - Convert path separators to /
 * - Drop trailing /index
 */
export function inferTitle(filePath: string): string {
  // Normalize slashes
  let path = filePath.replace(/\\/g, "/")

  // Remove leading slash
  if (path.startsWith("/")) {
    path = path.slice(1)
  }

  // Remove common roots (naive approach, customizable later)
  if (path.startsWith("src/")) {
    path = path.slice(4)
  }

  // Remove extensions
  // Matches .stories.ts, .stories.tsx, .story.ts, etc.
  path = path.replace(/\.stories\.(t|j)sx?$/, "")
  path = path.replace(/\.story\.(t|j)sx?$/, "")

  // Remove trailing /index
  if (path.endsWith("/index")) {
    path = path.slice(0, -6)
  }

  return path
}

/**
 * Generates a stable story ID from the title and export name.
 *
 * Format: fileSlug--exportName
 * fileSlug: lowercase title, slash -> dash, alphanumeric + dash only
 */
export function toId(title: string, exportName: string): string {
  const fileSlug = title
    .toLowerCase()
    .replace(/\//g, "-")
    .replace(/[^a-z0-9-]/g, "")

  const storySlug = exportName
    .replace(/([a-z0-9])([A-Z])/g, "$1-$2") // camel to kebab
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, "")

  return `${fileSlug}--${storySlug}`
}

/**
 * Generates a display name from the export name.
 * Uses startCase.
 */
export function storyNameFromExport(exportName: string): string {
  return startCase(exportName)
}
