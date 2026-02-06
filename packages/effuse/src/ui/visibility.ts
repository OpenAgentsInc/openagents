import type {
  AuthState,
  DataModel,
  LogicExpression,
  VisibilityCondition,
  DynamicValue,
} from "./types.js"
import { resolveDynamicValue } from "./data.js"

export type VisibilityContext = {
  dataModel: DataModel
  authState?: AuthState
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null

export const isLogicExpression = (value: unknown): value is LogicExpression => {
  if (!isRecord(value)) {
    return false
  }
  if ("and" in value || "or" in value) {
    const list = (value as { and?: unknown; or?: unknown }).and ??
      (value as { or?: unknown }).or
    return Array.isArray(list)
  }
  if ("not" in value) {
    return true
  }
  if ("path" in value) {
    return typeof (value as { path?: unknown }).path === "string"
  }
  if ("eq" in value || "neq" in value || "gt" in value || "gte" in value || "lt" in value || "lte" in value) {
    return true
  }
  return false
}

export const isVisibilityCondition = (value: unknown): value is VisibilityCondition => {
  if (typeof value === "boolean") {
    return true
  }
  if (!isRecord(value)) {
    return false
  }
  if ("path" in value && typeof value.path === "string") {
    return true
  }
  if ("auth" in value) {
    return value.auth === "signedIn" || value.auth === "signedOut"
  }
  return isLogicExpression(value)
}

export const evaluateLogicExpression = (
  expr: LogicExpression,
  ctx: VisibilityContext
): boolean => {
  const { dataModel } = ctx

  if ("and" in expr) {
    return expr.and.every((subExpr) => evaluateLogicExpression(subExpr, ctx))
  }

  if ("or" in expr) {
    return expr.or.some((subExpr) => evaluateLogicExpression(subExpr, ctx))
  }

  if ("not" in expr) {
    return !evaluateLogicExpression(expr.not, ctx)
  }

  if ("path" in expr) {
    const value = resolveDynamicValue({ path: expr.path }, dataModel)
    return Boolean(value)
  }

  if ("eq" in expr) {
    const [left, right] = expr.eq
    return resolveDynamicValue(left, dataModel) === resolveDynamicValue(right, dataModel)
  }

  if ("neq" in expr) {
    const [left, right] = expr.neq
    return resolveDynamicValue(left, dataModel) !== resolveDynamicValue(right, dataModel)
  }

  if ("gt" in expr) {
    const [left, right] = expr.gt
    const leftValue = resolveDynamicValue(left as DynamicValue<number>, dataModel)
    const rightValue = resolveDynamicValue(right as DynamicValue<number>, dataModel)
    return typeof leftValue === "number" && typeof rightValue === "number"
      ? leftValue > rightValue
      : false
  }

  if ("gte" in expr) {
    const [left, right] = expr.gte
    const leftValue = resolveDynamicValue(left as DynamicValue<number>, dataModel)
    const rightValue = resolveDynamicValue(right as DynamicValue<number>, dataModel)
    return typeof leftValue === "number" && typeof rightValue === "number"
      ? leftValue >= rightValue
      : false
  }

  if ("lt" in expr) {
    const [left, right] = expr.lt
    const leftValue = resolveDynamicValue(left as DynamicValue<number>, dataModel)
    const rightValue = resolveDynamicValue(right as DynamicValue<number>, dataModel)
    return typeof leftValue === "number" && typeof rightValue === "number"
      ? leftValue < rightValue
      : false
  }

  if ("lte" in expr) {
    const [left, right] = expr.lte
    const leftValue = resolveDynamicValue(left as DynamicValue<number>, dataModel)
    const rightValue = resolveDynamicValue(right as DynamicValue<number>, dataModel)
    return typeof leftValue === "number" && typeof rightValue === "number"
      ? leftValue <= rightValue
      : false
  }

  return false
}

export const evaluateVisibility = (
  condition: VisibilityCondition | undefined,
  ctx: VisibilityContext
): boolean => {
  if (condition === undefined) {
    return true
  }

  if (typeof condition === "boolean") {
    return condition
  }

  if ("path" in condition && !("and" in condition) && !("or" in condition)) {
    const value = resolveDynamicValue({ path: condition.path }, ctx.dataModel)
    return Boolean(value)
  }

  if ("auth" in condition) {
    const isSignedIn = ctx.authState?.isSignedIn ?? false
    return condition.auth === "signedIn" ? isSignedIn : !isSignedIn
  }

  return evaluateLogicExpression(condition as LogicExpression, ctx)
}
