import type { DataModel, DynamicValue, LogicExpression } from "./types.js"
import { resolveDynamicValue } from "./data.js"
import { evaluateLogicExpression } from "./visibility.js"

export interface ValidationCheck {
  fn: string
  args?: Record<string, DynamicValue>
  message: string
}

export interface ValidationConfig {
  checks?: ValidationCheck[]
  validateOn?: "change" | "blur" | "submit"
  enabled?: LogicExpression
}

export type ValidationFunction = (
  value: unknown,
  args?: Record<string, unknown>
) => boolean

export interface ValidationFunctionDefinition {
  validate: ValidationFunction
  description?: string
}

export interface ValidationCheckResult {
  fn: string
  valid: boolean
  message: string
}

export interface ValidationResult {
  valid: boolean
  errors: string[]
  checks: ValidationCheckResult[]
}

export interface ValidationContext {
  value: unknown
  dataModel: DataModel
  customFunctions?: Record<string, ValidationFunction>
}

export const builtInValidationFunctions: Record<string, ValidationFunction> = {
  required: (value) => {
    if (value === null || value === undefined) return false
    if (typeof value === "string") return value.trim().length > 0
    if (Array.isArray(value)) return value.length > 0
    return true
  },
  email: (value) => {
    if (typeof value !== "string") return false
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)
  },
  minLength: (value, args) => {
    if (typeof value !== "string") return false
    const min = args?.min
    return typeof min === "number" ? value.length >= min : false
  },
  maxLength: (value, args) => {
    if (typeof value !== "string") return false
    const max = args?.max
    return typeof max === "number" ? value.length <= max : false
  },
  pattern: (value, args) => {
    if (typeof value !== "string") return false
    const pattern = args?.pattern
    if (typeof pattern !== "string") return false
    try {
      return new RegExp(pattern).test(value)
    } catch {
      return false
    }
  },
  min: (value, args) => {
    if (typeof value !== "number") return false
    const min = args?.min
    return typeof min === "number" ? value >= min : false
  },
  max: (value, args) => {
    if (typeof value !== "number") return false
    const max = args?.max
    return typeof max === "number" ? value <= max : false
  },
  numeric: (value) => {
    if (typeof value === "number") return Number.isFinite(value)
    if (typeof value === "string") return !Number.isNaN(Number.parseFloat(value))
    return false
  },
  url: (value) => {
    if (typeof value !== "string") return false
    try {
      new URL(value)
      return true
    } catch {
      return false
    }
  },
  matches: (value, args) => {
    return value === args?.other
  },
}

export const runValidationCheck = (
  check: ValidationCheck,
  ctx: ValidationContext
): ValidationCheckResult => {
  const resolvedArgs: Record<string, unknown> = {}
  if (check.args) {
    for (const [key, argValue] of Object.entries(check.args)) {
      resolvedArgs[key] = resolveDynamicValue(argValue, ctx.dataModel)
    }
  }

  const fn =
    ctx.customFunctions?.[check.fn] ?? builtInValidationFunctions[check.fn]

  if (!fn) {
    return {
      fn: check.fn,
      valid: false,
      message: `Unknown validation function: ${check.fn}`,
    }
  }

  const valid = fn(ctx.value, resolvedArgs)
  return {
    fn: check.fn,
    valid,
    message: check.message,
  }
}

export const runValidationConfig = (
  config: ValidationConfig | undefined,
  ctx: ValidationContext
): ValidationResult => {
  if (!config?.checks || config.checks.length === 0) {
    return { valid: true, errors: [], checks: [] }
  }

  if (config.enabled) {
    const enabled = evaluateLogicExpression(config.enabled, {
      dataModel: ctx.dataModel,
    })
    if (!enabled) {
      return { valid: true, errors: [], checks: [] }
    }
  }

  const checks = config.checks.map((check) => runValidationCheck(check, ctx))
  const errors = checks.filter((check) => !check.valid).map((check) => check.message)
  return {
    valid: errors.length === 0,
    errors,
    checks,
  }
}
