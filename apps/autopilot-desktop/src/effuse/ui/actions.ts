import type { DataModel, DynamicValue } from "./types.js"
import { resolveDynamicValue } from "./data.js"

export interface ActionConfirm {
  title: string
  message: string
  confirmLabel?: string
  cancelLabel?: string
  variant?: "default" | "danger"
}

export type ActionOnSuccess =
  | { navigate: string }
  | { set: Record<string, unknown> }
  | { action: string }

export type ActionOnError =
  | { set: Record<string, unknown> }
  | { action: string }

export interface Action {
  name: string
  params?: Record<string, DynamicValue>
  confirm?: ActionConfirm
  onSuccess?: ActionOnSuccess
  onError?: ActionOnError
}

export type ActionHandler<TParams = Record<string, unknown>, TResult = unknown> = (
  params: TParams
) => Promise<TResult> | TResult

export interface ResolvedAction {
  name: string
  params: Record<string, unknown>
  confirm?: ActionConfirm
  onSuccess?: ActionOnSuccess
  onError?: ActionOnError
}

export const resolveAction = (action: Action, dataModel: DataModel): ResolvedAction => {
  const resolvedParams: Record<string, unknown> = {}

  if (action.params) {
    for (const [key, value] of Object.entries(action.params)) {
      resolvedParams[key] = resolveDynamicValue(value, dataModel)
    }
  }

  let confirm = action.confirm
  if (confirm) {
    confirm = {
      ...confirm,
      title: interpolateString(confirm.title, dataModel),
      message: interpolateString(confirm.message, dataModel),
    }
  }

  return {
    name: action.name,
    params: resolvedParams,
    confirm,
    onSuccess: action.onSuccess,
    onError: action.onError,
  }
}

export const interpolateString = (template: string, dataModel: DataModel): string =>
  template.replace(/\$\{([^}]+)\}/g, (_match, path) => {
    const value = resolveDynamicValue({ path }, dataModel)
    return String(value ?? "")
  })

export interface ActionExecutionContext {
  action: ResolvedAction
  handler: ActionHandler
  setData: (path: string, value: unknown) => void
  navigate?: (path: string) => void
  executeAction?: (name: string) => Promise<void>
}

export const executeAction = async (ctx: ActionExecutionContext): Promise<void> => {
  const { action, handler, setData, navigate, executeAction } = ctx

  await handler(action.params)

  if (action.onSuccess) {
    if ("navigate" in action.onSuccess && navigate) {
      navigate(action.onSuccess.navigate)
    } else if ("set" in action.onSuccess) {
      for (const [path, value] of Object.entries(action.onSuccess.set)) {
        setData(path, value)
      }
    } else if ("action" in action.onSuccess && executeAction) {
      await executeAction(action.onSuccess.action)
    }
  }
}

export const executeActionWithErrorHandling = async (
  ctx: ActionExecutionContext,
  onError?: (error: Error) => void
): Promise<void> => {
  try {
    await executeAction(ctx)
  } catch (error) {
    const err = error instanceof Error ? error : new Error(String(error))
    if (ctx.action.onError) {
      if ("set" in ctx.action.onError) {
        for (const [path, value] of Object.entries(ctx.action.onError.set)) {
          const resolvedValue =
            typeof value === "string" && value === "$error.message"
              ? err.message
              : value
          ctx.setData(path, resolvedValue)
        }
      } else if ("action" in ctx.action.onError && ctx.executeAction) {
        await ctx.executeAction(ctx.action.onError.action)
      }
    } else if (onError) {
      onError(err)
    } else {
      throw err
    }
  }
}
