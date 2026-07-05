export const khalaCopyKeys = [
  "app.title",
  "errorBoundary.body",
  "errorBoundary.help",
  "errorBoundary.retry",
  "nav.settings",
  "nav.threads",
  "signIn.discovery.help",
  "signIn.discovery.looking",
  "signIn.discovery.noSignedInMac",
  "signIn.discovery.reachableNotSignedIn",
  "signIn.discovery.reachableNotSignedInOnHost",
  "signIn.manual.backToDiscovery",
  "signIn.manual.ownerUserId",
  "signIn.manual.submit",
  "signIn.manual.subtitle",
  "signIn.manual.token",
  "signIn.manualInstead",
  "signIn.retry",
] as const

export type KhalaCopyKey = (typeof khalaCopyKeys)[number]

export const enCopy = {
  "app.title": "Khala Code",
  "errorBoundary.body": "Something went wrong in this mobile view.",
  "errorBoundary.help": "Try again or reopen the app.",
  "errorBoundary.retry": "Try again",
  "nav.settings": "Settings",
  "nav.threads": "Khala",
  "signIn.discovery.help":
    "Make sure Tailscale is connected on both this phone and your Mac, and Khala Code is running and signed in there.",
  "signIn.discovery.looking": "Looking for a signed-in Mac on your Tailnet...",
  "signIn.discovery.noSignedInMac": "No signed-in Mac found on your Tailnet.",
  "signIn.discovery.reachableNotSignedIn":
    "Found Khala Code, but it has not completed Connect OpenAgents yet. Open Khala Code on your Mac, finish Connect, then retry.",
  "signIn.discovery.reachableNotSignedInOnHost":
    "Found Khala Code on {hostname}, but it has not completed Connect OpenAgents yet. Open Khala Code on your Mac, finish Connect, then retry.",
  "signIn.manual.backToDiscovery": "Back to Tailnet auto-discovery",
  "signIn.manual.ownerUserId": "Owner user id",
  "signIn.manual.submit": "Sign in",
  "signIn.manual.subtitle": "Sign in with your OpenAgents account to sync chats and your fleet.",
  "signIn.manual.token": "OpenAgents token",
  "signIn.manualInstead": "Sign in manually instead",
  "signIn.retry": "Retry",
} satisfies Record<KhalaCopyKey, string>

export const copyLocale = "en" as const

const copyTable: Record<KhalaCopyKey, string> = enCopy

export const tx = (
  key: KhalaCopyKey,
  params: Readonly<Record<string, string | number>> = {},
): string => {
  const template = copyTable[key]
  if (template === undefined) {
    throw new Error(`Missing Khala mobile copy key: ${key}`)
  }
  return template.replace(/\{([A-Za-z0-9_]+)\}/g, (_match, param: string) => {
    const value = params[param]
    if (value === undefined) {
      throw new Error(`Missing Khala mobile copy param "${param}" for key: ${key}`)
    }
    return String(value)
  })
}
