export const DesktopRendererScheme = "openagents-app"

export const desktopRendererEntryUrl = `${DesktopRendererScheme}://renderer/index.html`

/** Renderer IPC authority is one exact entry URL in every mode. */
export const isTrustedDesktopRendererUrl = (input: Readonly<{
  trustedEntryUrl: string
  value: string
}>): boolean => input.value === input.trustedEntryUrl
