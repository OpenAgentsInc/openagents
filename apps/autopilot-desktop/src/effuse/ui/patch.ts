import type { JsonPatch, UITree, UIElement } from "./types.js"
import { setByPath } from "./data.js"

export const parsePatchLine = (line: string): JsonPatch | null => {
  try {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith("//")) {
      return null
    }
    return JSON.parse(trimmed) as JsonPatch
  } catch {
    return null
  }
}

export const applyPatch = (tree: UITree, patch: JsonPatch): UITree => {
  const nextTree: UITree = { ...tree, elements: { ...tree.elements } }

  switch (patch.op) {
    case "set":
    case "add":
    case "replace": {
      if (patch.path === "/root") {
        nextTree.root = patch.value as string
        return nextTree
      }

      if (patch.path.startsWith("/elements/")) {
        const pathParts = patch.path.slice("/elements/".length).split("/")
        const elementKey = pathParts[0]
        if (!elementKey) {
          return nextTree
        }

        if (pathParts.length === 1) {
          nextTree.elements[elementKey] = patch.value as UIElement
        } else {
          const element = nextTree.elements[elementKey]
          if (element) {
            const propPath = `/${pathParts.slice(1).join("/")}`
            const updated = { ...element }
            setByPath(updated as Record<string, unknown>, propPath, patch.value)
            nextTree.elements[elementKey] = updated
          }
        }
      }
      break
    }
    case "remove": {
      if (patch.path.startsWith("/elements/")) {
        const elementKey = patch.path.slice("/elements/".length).split("/")[0]
        if (elementKey) {
          const { [elementKey]: _, ...rest } = nextTree.elements
          nextTree.elements = rest
        }
      }
      break
    }
  }

  return nextTree
}

export const createEmptyTree = (): UITree => ({ root: "", elements: {} })
