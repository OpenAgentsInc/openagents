export type MobileWorkspaceKeyboardCommand = "new_task" | "navigation" | "detail" | "dismiss"

export const mobileWorkspaceKeyboardCommand = (input: Readonly<{
  key?: string
  metaKey?: boolean
  ctrlKey?: boolean
}>): MobileWorkspaceKeyboardCommand | null => {
  const key = input.key?.toLocaleLowerCase()
  const command = input.metaKey === true || input.ctrlKey === true
  if (!command) return key === "escape" ? "dismiss" : null
  if (key === "n") return "new_task"
  if (key === "k" || key === "1") return "navigation"
  if (key === "2") return "detail"
  return null
}
