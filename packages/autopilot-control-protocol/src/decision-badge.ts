export type DecisionBadgeTone = "none" | "attention"

export function decisionBadge(pendingCount: number): {
  show: boolean
  label: string
  tone: DecisionBadgeTone
} {
  if (pendingCount > 0) {
    return {
      show: true,
      label: `${pendingCount} need you`,
      tone: "attention",
    }
  }

  return {
    show: false,
    label: "",
    tone: "none",
  }
}
