export const khalaCopyKeys = [
  "app.title",
  "errorBoundary.body",
  "errorBoundary.help",
  "errorBoundary.retry",
  "nav.settings",
  "nav.threads",
  "signIn.github.primary",
  "signIn.github.subtitle",
] as const

export type KhalaCopyKey = (typeof khalaCopyKeys)[number]

export const enCopy = {
  "app.title": "Khala Code",
  "errorBoundary.body": "Something went wrong in this mobile view.",
  "errorBoundary.help": "Try again or reopen the app.",
  "errorBoundary.retry": "Try again",
  "nav.settings": "Settings",
  "nav.threads": "Khala",
  "signIn.github.primary": "Sign in with GitHub",
  "signIn.github.subtitle": "Connect GitHub to pick a repo and run Khala Code from this phone.",
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
