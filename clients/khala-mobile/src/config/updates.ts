export const KHALA_MOBILE_UPDATES_OWNER = "khala-mobile"
export const KHALA_MOBILE_UPDATES_URL =
  "https://updates.openagents.com/khala-mobile/manifest"
export const KHALA_MOBILE_UPDATE_CHANNEL = "production"

export const KHALA_MOBILE_OTA_CONTRACT = {
  build: "local-prebuild-xcode-gradle",
  channel: KHALA_MOBILE_UPDATE_CHANNEL,
  owner: KHALA_MOBILE_UPDATES_OWNER,
  url: KHALA_MOBILE_UPDATES_URL
} as const

export const forbiddenHostedExpoCommands = [
  "eas build",
  "eas submit",
  "eas update"
] as const
