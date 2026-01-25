export type DynamicValue<T = unknown> = T | { path: string }

export type DynamicString = DynamicValue<string>
export type DynamicNumber = DynamicValue<number>
export type DynamicBoolean = DynamicValue<boolean>

export type LogicExpression =
  | { and: LogicExpression[] }
  | { or: LogicExpression[] }
  | { not: LogicExpression }
  | { path: string }
  | { eq: [DynamicValue, DynamicValue] }
  | { neq: [DynamicValue, DynamicValue] }
  | { gt: [DynamicNumber, DynamicNumber] }
  | { gte: [DynamicNumber, DynamicNumber] }
  | { lt: [DynamicNumber, DynamicNumber] }
  | { lte: [DynamicNumber, DynamicNumber] }

export type VisibilityCondition =
  | boolean
  | { path: string }
  | { auth: "signedIn" | "signedOut" }
  | LogicExpression

export interface UIElement<T extends string = string, P = Record<string, unknown>> {
  key: string
  type: T
  props: P
  children?: string[]
  parentKey?: string | null
  visible?: VisibilityCondition
}

export interface UITree {
  root: string
  elements: Record<string, UIElement>
}

export type DataModel = Record<string, unknown>

export interface AuthState {
  isSignedIn: boolean
  user?: Record<string, unknown>
}

export type PatchOp = "add" | "remove" | "replace" | "set"

export interface JsonPatch {
  op: PatchOp
  path: string
  value?: unknown
}

export type ValidationMode = "strict" | "warn" | "ignore"
