import { join } from "node:path"

/**
 * Runtime supervisors execute owner work and therefore default to the account
 * registry rooted under their declared Pylon home. The general Pylon CLI may
 * discover `~/.codex*` / `~/.claude*` sibling homes for inventory, but a
 * supervisor must never widen custody to the owner's default CLI home merely
 * because `PYLON_ACCOUNT_HOME_ROOT` was omitted.
 *
 * An operator can still opt into a different bounded discovery root
 * explicitly. The caller passes the returned environment only to sibling
 * discovery; provider child processes continue to receive their normal env.
 */
export const runtimeSiblingAccountDiscoveryEnv = (
  pylonHome: string,
  env: Readonly<Record<string, string | undefined>>,
): Record<string, string | undefined> => ({
  ...env,
  PYLON_ACCOUNT_HOME_ROOT:
    env.PYLON_ACCOUNT_HOME_ROOT?.trim() || join(pylonHome, "accounts"),
})
