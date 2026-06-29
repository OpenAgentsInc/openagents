import type { OpenAgentsWorkerConfigEnv } from './config'

export const optionalMdkContainerSecret = (
  value: string | undefined,
): string | undefined => {
  const trimmed = value?.trim()

  return trimmed === undefined || trimmed === '' ? undefined : trimmed
}

export const mdkContainerEnvVars = (
  environment: OpenAgentsWorkerConfigEnv,
): Record<string, string> => {
  const accessToken = optionalMdkContainerSecret(environment.MDK_ACCESS_TOKEN)
  const mnemonic = optionalMdkContainerSecret(environment.MDK_MNEMONIC)
  const webhookSecret = optionalMdkContainerSecret(
    environment.MDK_WEBHOOK_SECRET,
  )
  const withdrawalDestination = optionalMdkContainerSecret(
    environment.WITHDRAWAL_DESTINATION,
  )

  return {
    ...(accessToken === undefined ? {} : { MDK_ACCESS_TOKEN: accessToken }),
    ...(mnemonic === undefined ? {} : { MDK_MNEMONIC: mnemonic }),
    ...(webhookSecret === undefined
      ? {}
      : { MDK_WEBHOOK_SECRET: webhookSecret }),
    ...(withdrawalDestination === undefined
      ? {}
      : { WITHDRAWAL_DESTINATION: withdrawalDestination }),
  }
}
