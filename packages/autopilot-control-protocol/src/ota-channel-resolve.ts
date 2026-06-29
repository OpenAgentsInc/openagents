export type OtaChannelResolveInput = {
  releaseChannel: string | null
  isProduction: boolean
}

export type OtaChannelResolution = {
  channel: string
  branch: string
  reason: string
}

const sanitizeReleaseChannel = (releaseChannel: string): string =>
  releaseChannel
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "")

export const resolveOtaChannel = (
  input: OtaChannelResolveInput,
): OtaChannelResolution => {
  if (input.isProduction) {
    return {
      channel: "production",
      branch: "production",
      reason: "production-build",
    }
  }

  if (input.releaseChannel !== null) {
    const channel = sanitizeReleaseChannel(input.releaseChannel)

    if (channel.length > 0) {
      return {
        channel,
        branch: channel,
        reason: "explicit-release-channel",
      }
    }
  }

  return {
    channel: "preview",
    branch: "preview",
    reason: "default-preview",
  }
}
