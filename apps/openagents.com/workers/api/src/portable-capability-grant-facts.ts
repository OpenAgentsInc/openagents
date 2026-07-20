import type { GitHubWriteRepository } from './github-write-connections'
import type { ProviderAccountRepository } from './provider-account-domain'

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u
const MAX_GRANTS = 32

export type PortableCapabilityGrantFact = Readonly<{
  grantRef: string
  kind: 'provider' | 'github'
  ownerUserId: string
  status: 'issued'
  expiresAt: string
  providerAccountRef?: string | undefined
  runnerSessionId?: string | undefined
}>

export class PortableCapabilityGrantFactError extends Error {
  override readonly name = 'PortableCapabilityGrantFactError'
}

export const resolvePortableCapabilityGrantFacts = async (input: Readonly<{
  ownerUserId: string
  grantRefs: ReadonlyArray<string>
  provider: Pick<ProviderAccountRepository, 'findGrantByRef' | 'findAccountByRef'>
  github: Pick<GitHubWriteRepository, 'findGrantByRef' | 'findUsableConnectionForUser'>
  now?: () => Date
}>): Promise<ReadonlyArray<PortableCapabilityGrantFact>> => {
  if (
    !SAFE_REF.test(input.ownerUserId) ||
    input.grantRefs.length === 0 ||
    input.grantRefs.length > MAX_GRANTS ||
    input.grantRefs.some(ref => !SAFE_REF.test(ref)) ||
    new Set(input.grantRefs).size !== input.grantRefs.length
  ) {
    throw new PortableCapabilityGrantFactError('portable capability grant fact scope is invalid')
  }
  const now = (input.now ?? (() => new Date()))().getTime()
  return Promise.all(input.grantRefs.map(async grantRef => {
    const [providerGrant, githubGrant] = await Promise.all([
      input.provider.findGrantByRef(grantRef),
      input.github.findGrantByRef(grantRef),
    ])
    if ((providerGrant === undefined) === (githubGrant === undefined)) {
      throw new PortableCapabilityGrantFactError('portable capability grant is absent or ambiguous')
    }
    if (providerGrant !== undefined) {
      if (
        providerGrant.userId !== input.ownerUserId ||
        providerGrant.status !== 'issued' ||
        Date.parse(providerGrant.expiresAt) <= now
      ) throw new PortableCapabilityGrantFactError('provider capability grant is not active')
      const account = await input.provider.findAccountByRef(
        input.ownerUserId,
        providerGrant.providerAccountRef,
      )
      if (
        account === undefined || account.status !== 'connected' || account.health !== 'healthy' ||
        account.providerAccountRef !== providerGrant.providerAccountRef ||
        account.secretRef !== providerGrant.providerSecretRef
      ) throw new PortableCapabilityGrantFactError('provider account is not usable')
      return {
        grantRef,
        kind: 'provider',
        ownerUserId: input.ownerUserId,
        status: 'issued',
        expiresAt: providerGrant.expiresAt,
        providerAccountRef: providerGrant.providerAccountRef,
        ...(providerGrant.runnerSessionId === null ? {} : { runnerSessionId: providerGrant.runnerSessionId }),
      }
    }
    if (githubGrant === undefined) {
      throw new PortableCapabilityGrantFactError('portable capability grant is absent')
    }
    if (
      githubGrant.userId !== input.ownerUserId || githubGrant.status !== 'issued' ||
      Date.parse(githubGrant.expiresAt) <= now
    ) throw new PortableCapabilityGrantFactError('GitHub capability grant is not active')
    const connection = await input.github.findUsableConnectionForUser(input.ownerUserId)
    if (
      connection === undefined || connection.connectionRef !== githubGrant.connectionRef ||
      connection.secretRef !== githubGrant.secretRef
    ) throw new PortableCapabilityGrantFactError('GitHub connection is not usable')
    return {
      grantRef,
      kind: 'github',
      ownerUserId: input.ownerUserId,
      status: 'issued',
      expiresAt: githubGrant.expiresAt,
      ...(githubGrant.runnerSessionId === null ? {} : { runnerSessionId: githubGrant.runnerSessionId }),
    }
  }))
}
