import type {
  AgentRunAssignment,
  AppDeployAssignment,
} from '@openagentsinc/sync-schema'
import { Effect, Layer } from 'effect'

import { OpenAgentsDatabase } from '../bindings'
import {
  OpenAgentsWorkerConfig,
  type OpenAgentsWorkerConfigEnv,
} from '../config'
import type { DispatchResult } from '../omni-runs'
import {
  type OmniDispatchConfig,
  OmniDispatchService,
  makeOmniDispatchService,
} from '../omni/dispatch-service'
import {
  OpenAiCodexProviderClient,
  type OpenAiCodexProviderClientShape,
} from '../provider-account-client'
import {
  CHATGPT_CODEX_VERIFICATION_URL,
  type CodexDeviceLoginPollResult,
  type IdFactory,
  type ProviderAccountRepository,
  type StartedCodexDeviceLogin,
  type StartedCodexDeviceLoginSecret,
} from '../provider-account-domain'
import {
  ProviderAccountRepositoryService,
  makeProviderAccountRepositoryService,
} from '../provider-account-repository'
import {
  ProviderAccountLifecycleService,
  type ProviderAccountLifecycleServiceDependencies,
  makeProviderAccountLifecycleService,
} from '../provider-account-service'
export const testOpenAgentsWorkerConfigEnv = (
  overrides: OpenAgentsWorkerConfigEnv = {},
): OpenAgentsWorkerConfigEnv => ({
  GITHUB_CLIENT_ID: 'github-client',
  GITHUB_CLIENT_SECRET: 'github-secret',
  OPENAGENTS_APP_URL: 'https://openagents.com',
  OPENAUTH_CLIENT_ID: 'openauth-client',
  OPENAUTH_ISSUER_URL: 'https://openagents.com',
  ...overrides,
})

export const makeOpenAgentsWorkerConfigTestLayer = (
  overrides: OpenAgentsWorkerConfigEnv = {},
) => OpenAgentsWorkerConfig.layer(testOpenAgentsWorkerConfigEnv(overrides))

export const makeOpenAgentsDatabaseTestLayer = (db: D1Database) =>
  Layer.succeed(OpenAgentsDatabase, db)

export const makeProviderAccountRepositoryTestLayer = (
  repository: ProviderAccountRepository,
) =>
  Layer.succeed(
    ProviderAccountRepositoryService,
    makeProviderAccountRepositoryService(repository),
  )

export const providerAccountTestNow = () => new Date('2026-06-02T19:00:00.000Z')

export const providerAccountTestIdFactory =
  (values: ReadonlyArray<string> = []): IdFactory =>
  prefix => {
    const next = values[0] ?? prefix

    values = values.slice(1)

    return `${prefix}_${next}`
  }

export const testStartedCodexDeviceLogin = (
  overrides: Partial<StartedCodexDeviceLogin> = {},
): StartedCodexDeviceLogin => ({
  deviceAuthId: 'device_auth_1',
  expiresAt: '2026-06-02T19:15:00.000Z',
  intervalSeconds: 5,
  userCode: 'ABCD-EFGH',
  verificationUrl: CHATGPT_CODEX_VERIFICATION_URL,
  ...overrides,
})

export const testStartedCodexDeviceLoginSecret = (
  overrides: Partial<StartedCodexDeviceLoginSecret> = {},
): StartedCodexDeviceLoginSecret => ({
  deviceAuthId: 'device_auth_1',
  userCode: 'ABCD-EFGH',
  ...overrides,
})

export const makeOpenAiCodexProviderClientTestLayer = (
  options: Readonly<{
    deviceLogin?: StartedCodexDeviceLogin
    pollResult?: CodexDeviceLoginPollResult
  }> = {},
) => {
  const client: OpenAiCodexProviderClientShape = {
    pollDeviceLogin: () =>
      Effect.succeed(options.pollResult ?? { status: 'pending' }),
    startDeviceLogin: () =>
      Effect.succeed(options.deviceLogin ?? testStartedCodexDeviceLogin()),
  }

  return Layer.succeed(OpenAiCodexProviderClient, client)
}

export const makeProviderAccountLifecycleTestLayer = (
  dependencies: Readonly<{
    makeId?: IdFactory
    now?: () => Date
    pollResult?: CodexDeviceLoginPollResult
    repository: ProviderAccountRepository
    startedDeviceLogin?: StartedCodexDeviceLogin
    startedSecret?: StartedCodexDeviceLoginSecret
  }>,
) => {
  const startedSecrets = new Map<string, StartedCodexDeviceLoginSecret>()
  const serviceDependencies: ProviderAccountLifecycleServiceDependencies = {
    deleteStartedDeviceLogin: async attemptId => {
      startedSecrets.delete(attemptId)
    },
    makeId: dependencies.makeId,
    now: dependencies.now ?? providerAccountTestNow,
    pollDeviceLogin: async () =>
      dependencies.pollResult ?? { status: 'pending' },
    readStartedDeviceLogin: async attemptId =>
      startedSecrets.get(attemptId) ??
      dependencies.startedSecret ??
      testStartedCodexDeviceLoginSecret(),
    repository: dependencies.repository,
    startDeviceLogin: async () =>
      dependencies.startedDeviceLogin ?? testStartedCodexDeviceLogin(),
    storeConnectedAuth: async ({ providerAccountRef }) =>
      `codex-auth://${providerAccountRef}`,
    storeStartedDeviceLogin: async input => {
      startedSecrets.set(input.attemptId, {
        deviceAuthId: input.deviceAuthId,
        userCode: input.userCode,
      })
    },
  }

  return Layer.succeed(
    ProviderAccountLifecycleService,
    makeProviderAccountLifecycleService(serviceDependencies),
  )
}

export type OmniDispatchCall =
  | Readonly<{
      assignment: AgentRunAssignment
      config: OmniDispatchConfig
      kind: 'agent-run'
    }>
  | Readonly<{
      assignment: AppDeployAssignment
      config: OmniDispatchConfig
      kind: 'deployment'
    }>

export const makeOmniDispatchServiceTestLayer = (
  result: DispatchResult = {
    externalId: 'shc:memory:run',
    mode: 'live',
    status: 'queued',
  },
) => {
  const calls: Array<OmniDispatchCall> = []
  const service = makeOmniDispatchService({
    dispatchAgentRunToShc: async (assignment, config) => {
      calls.push({ assignment, config, kind: 'agent-run' })

      return result
    },
    dispatchDeploymentToShc: async (assignment, config) => {
      calls.push({ assignment, config, kind: 'deployment' })

      return result
    },
  })

  return {
    calls,
    layer: Layer.succeed(OmniDispatchService, service),
    service,
  }
}

// CFG-7 (#8522): the RUNNER_EVENTS queue fixture was deleted with the dead
// RUNNER_EVENTS Cloudflare Queue lane (no producers, no consumer).
