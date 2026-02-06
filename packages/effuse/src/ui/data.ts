import type { DataModel, DynamicValue } from "./types.js"

const isPathValue = (value: unknown): value is { path: string } => {
  if (!value || typeof value !== "object") {
    return false
  }
  if (!("path" in value) || typeof (value as { path?: unknown }).path !== "string") {
    return false
  }
  return Object.keys(value as Record<string, unknown>).length === 1
}

export const getByPath = (obj: unknown, path: string): unknown => {
  if (!path || path === "/") {
    return obj
  }

  const segments = path.startsWith("/") ? path.slice(1).split("/") : path.split("/")
  let current: unknown = obj

  for (const segment of segments) {
    if (current === null || current === undefined) {
      return undefined
    }
    if (typeof current === "object") {
      current = (current as Record<string, unknown>)[segment]
    } else {
      return undefined
    }
  }

  return current
}

export const setByPath = (
  obj: Record<string, unknown>,
  path: string,
  value: unknown
): void => {
  const segments = path.startsWith("/") ? path.slice(1).split("/") : path.split("/")
  if (segments.length === 0) {
    return
  }

  let current: Record<string, unknown> = obj
  for (let i = 0; i < segments.length - 1; i += 1) {
    const segment = segments[i]
    if (!segment) {
      continue
    }
    const existing = current[segment]
    if (!existing || typeof existing !== "object") {
      current[segment] = {}
    }
    current = current[segment] as Record<string, unknown>
  }

  const lastSegment = segments[segments.length - 1]
  if (lastSegment) {
    current[lastSegment] = value
  }
}

export const resolveDynamicValue = <T>(
  value: DynamicValue<T>,
  dataModel: DataModel
): T | undefined => {
  if (value === null || value === undefined) {
    return undefined
  }

  if (isPathValue(value)) {
    return getByPath(dataModel, value.path) as T | undefined
  }

  return value as T
}

export const resolveDynamicObject = (
  value: unknown,
  dataModel: DataModel
): unknown => {
  if (isPathValue(value)) {
    return resolveDynamicValue(value, dataModel)
  }

  if (Array.isArray(value)) {
    return value.map((item) => resolveDynamicObject(item, dataModel))
  }

  if (value && typeof value === "object") {
    const resolved: Record<string, unknown> = {}
    for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
      resolved[key] = resolveDynamicObject(child, dataModel)
    }
    return resolved
  }

  return value
}
