export const isMacOs = (): boolean => {
  if (typeof navigator !== "undefined") {
    return navigator.platform.toUpperCase().indexOf("MAC") >= 0
  }
  // Default if platform cannot be determined
  return false
}

export const getModifierKey = (): string => {
  return isMacOs() ? "⌘" : "Ctrl"
}
