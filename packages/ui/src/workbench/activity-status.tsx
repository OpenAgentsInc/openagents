import { Check, Circle, CircleDot, Pause, XCircle } from "lucide-react"
import type { ReactElement } from "react"

/**
 * Shared lifecycle-status vocabulary consumed by the protocol/tool-call cards
 * and the agent group rows. Public so Wave-2 lanes (T4-T12) can key their own
 * cards off the same status set.
 */
export type DesktopActivityStatus = "completed" | "failed" | "pending" | "running" | "waiting"

export const activityStatusLabel = (status: DesktopActivityStatus): string => status === "completed"
  ? "Done"
  : status === "failed"
    ? "Failed"
    : status === "running"
      ? "Running"
      : status === "waiting"
        ? "Waiting"
        : "Pending"

export const activityStatusIcon = (status: DesktopActivityStatus): ReactElement => {
  if (status === "completed") return <Check aria-hidden="true" />
  if (status === "failed") return <XCircle aria-hidden="true" />
  if (status === "running") return <CircleDot aria-hidden="true" />
  if (status === "waiting") return <Pause aria-hidden="true" />
  return <Circle aria-hidden="true" />
}
