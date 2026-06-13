export type IntentStatus = "received" | "planning" | "fanning_out" | "shipping" | "shipped" | "failed"

export type ShipStatusTone = "info" | "warning" | "success" | "danger"

export type ShipStatusViewModel = {
  label: string
  tone: ShipStatusTone
  stepIndex: number
  totalSteps: number
}

const TOTAL_STEPS = 5

const STATUS_ROWS: Readonly<Record<IntentStatus, ShipStatusViewModel>> = {
  received: {
    label: "Received",
    tone: "info",
    stepIndex: 1,
    totalSteps: TOTAL_STEPS,
  },
  planning: {
    label: "Planning",
    tone: "info",
    stepIndex: 2,
    totalSteps: TOTAL_STEPS,
  },
  fanning_out: {
    label: "Fanning out",
    tone: "warning",
    stepIndex: 3,
    totalSteps: TOTAL_STEPS,
  },
  shipping: {
    label: "Shipping",
    tone: "warning",
    stepIndex: 4,
    totalSteps: TOTAL_STEPS,
  },
  shipped: {
    label: "Shipped",
    tone: "success",
    stepIndex: 5,
    totalSteps: TOTAL_STEPS,
  },
  failed: {
    label: "Failed",
    tone: "danger",
    stepIndex: 5,
    totalSteps: TOTAL_STEPS,
  },
}

export function shipStatusView(status: IntentStatus): ShipStatusViewModel {
  return STATUS_ROWS[status]
}
