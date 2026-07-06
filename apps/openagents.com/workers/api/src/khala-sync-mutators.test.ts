import { describe, expect, test } from 'vitest'

import {
  ClientGroupId,
  ClientId,
  MutationId,
  MutatorName,
  personalScope,
} from '@openagentsinc/khala-sync'
import type {
  AppendChangeInput,
  MutatorContext,
  SyncTransactionWriter,
} from '@openagentsinc/khala-sync-server'
import {
  CHAT_APPEND_MESSAGE_MUTATOR_NAME,
  CHAT_BIND_THREAD_REPO_MUTATOR_NAME,
  CHAT_CREATE_THREAD_MUTATOR_NAME,
  CHAT_RENAME_THREAD_MUTATOR_NAME,
  RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME,
  RUNTIME_CLOSE_TURN_MUTATOR_NAME,
  RUNTIME_CONTINUE_TURN_MUTATOR_NAME,
  RUNTIME_INTERRUPT_TURN_MUTATOR_NAME,
  RUNTIME_RECORD_EVENT_MUTATOR_NAME,
  RUNTIME_RETRY_TURN_MUTATOR_NAME,
  RUNTIME_START_TURN_MUTATOR_NAME,
} from '@openagentsinc/khala-sync-server'

import {
  debugEchoMutator,
  decodeDebugEchoArgs,
  makeKhalaSyncWorkerMutatorRegistry,
  SYNC_DEBUG_ECHO_ENTITY_TYPE,
  SYNC_DEBUG_ECHO_MUTATOR_NAME,
  SYNC_DEBUG_ECHO_SCOPE_REJECTION,
} from './khala-sync-mutators'

const makeFakeWriter = () => {
  const appended: Array<AppendChangeInput> = []
  const writer: SyncTransactionWriter = {
    allocateVersion: () => {
      throw new Error('allocateVersion not used by debugEcho')
    },
    appendChange: async change => {
      appended.push(change)
      return undefined as never
    },
    sql: (() => {
      throw new Error('debugEcho must not issue raw SQL')
    }) as never,
  }
  return { appended, writer }
}

const makeCtx = (userId: string, writer: SyncTransactionWriter): MutatorContext => ({
  clientGroupId: ClientGroupId.make('cg-1'),
  clientId: ClientId.make('c-1'),
  mutationId: MutationId.make(7),
  mutationRef: 'mutation:cg-1:c-1:7',
  userId,
  writer,
})

describe('khala sync worker mutator registry', () => {
  test('the registry carries sync.debugEcho, chat, runtime, and fleet operator mutators', () => {
    const registry = makeKhalaSyncWorkerMutatorRegistry()
    expect(registry.names().map(String)).toEqual([
      SYNC_DEBUG_ECHO_MUTATOR_NAME,
      CHAT_CREATE_THREAD_MUTATOR_NAME,
      CHAT_APPEND_MESSAGE_MUTATOR_NAME,
      CHAT_RENAME_THREAD_MUTATOR_NAME,
      CHAT_BIND_THREAD_REPO_MUTATOR_NAME,
      RUNTIME_START_TURN_MUTATOR_NAME,
      RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME,
      RUNTIME_INTERRUPT_TURN_MUTATOR_NAME,
      RUNTIME_CONTINUE_TURN_MUTATOR_NAME,
      RUNTIME_RETRY_TURN_MUTATOR_NAME,
      RUNTIME_CLOSE_TURN_MUTATOR_NAME,
      RUNTIME_RECORD_EVENT_MUTATOR_NAME,
      'fleet.setDesiredSlots',
      'fleet.pauseRun',
      'fleet.resumeRun',
      'fleet.pauseWorker',
      'fleet.resumeWorker',
      'fleet.acknowledgeInboxFlag',
      'fleet.stopRun',
      'fleet.reportAccountState',
    ])
    expect(
      registry.get(MutatorName.make(SYNC_DEBUG_ECHO_MUTATOR_NAME)),
    ).toBeDefined()
    for (const name of [
      CHAT_CREATE_THREAD_MUTATOR_NAME,
      CHAT_APPEND_MESSAGE_MUTATOR_NAME,
      CHAT_RENAME_THREAD_MUTATOR_NAME,
      CHAT_BIND_THREAD_REPO_MUTATOR_NAME,
    ]) {
      expect(registry.get(MutatorName.make(name))).toBeDefined()
    }
    for (const name of [
      RUNTIME_START_TURN_MUTATOR_NAME,
      RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME,
      RUNTIME_INTERRUPT_TURN_MUTATOR_NAME,
      RUNTIME_CONTINUE_TURN_MUTATOR_NAME,
      RUNTIME_RETRY_TURN_MUTATOR_NAME,
      RUNTIME_CLOSE_TURN_MUTATOR_NAME,
      RUNTIME_RECORD_EVENT_MUTATOR_NAME,
    ]) {
      expect(registry.get(MutatorName.make(name))).toBeDefined()
    }
    for (const name of [
      'fleet.setDesiredSlots',
      'fleet.pauseRun',
      'fleet.resumeRun',
      'fleet.pauseWorker',
      'fleet.resumeWorker',
      'fleet.acknowledgeInboxFlag',
      'fleet.stopRun',
      'fleet.reportAccountState',
    ]) {
      expect(registry.get(MutatorName.make(name))).toBeDefined()
    }
  })

  test('decodeDebugEchoArgs rejects malformed args', () => {
    expect(() => decodeDebugEchoArgs('not json')).toThrow()
    expect(() => decodeDebugEchoArgs(JSON.stringify({ echo: 'x' }))).toThrow()
    expect(() =>
      decodeDebugEchoArgs(
        JSON.stringify({ scope: 'not-a-scope', entityId: 'e', echo: 'x' }),
      ),
    ).toThrow()
    expect(() =>
      decodeDebugEchoArgs(
        JSON.stringify({ scope: 'scope.user.u1', entityId: '', echo: 'x' }),
      ),
    ).toThrow()
  })

  test('debugEcho rejects a foreign scope IN-BAND without writing anything', async () => {
    const { appended, writer } = makeFakeWriter()
    const args = decodeDebugEchoArgs(
      JSON.stringify({
        echo: 'hi',
        entityId: 'e-1',
        scope: 'scope.user.someone-else',
      }),
    )
    const result = await debugEchoMutator.execute(args, makeCtx('u-1', writer))
    expect(result.status).toBe('rejected')
    expect(result.errorCode).toBe(SYNC_DEBUG_ECHO_SCOPE_REJECTION)
    expect(Number(result.mutationId)).toBe(7)
    expect(appended).toEqual([])
  })

  test('debugEcho rejects team/public scopes too (personal scope only)', async () => {
    const { appended, writer } = makeFakeWriter()
    const args = decodeDebugEchoArgs(
      JSON.stringify({
        echo: 'hi',
        entityId: 'e-1',
        scope: 'scope.team.u-1',
      }),
    )
    const result = await debugEchoMutator.execute(args, makeCtx('u-1', writer))
    expect(result.status).toBe('rejected')
    expect(appended).toEqual([])
  })

  test('debugEcho writes one upsert into the caller’s personal scope with the mutation ref', async () => {
    const { appended, writer } = makeFakeWriter()
    const args = decodeDebugEchoArgs(
      JSON.stringify({
        echo: 'khala hears you',
        entityId: 'e-42',
        scope: 'scope.user.u-1',
      }),
    )
    const result = await debugEchoMutator.execute(args, makeCtx('u-1', writer))
    expect(result.status).toBe('applied')
    expect(Number(result.mutationId)).toBe(7)
    expect(appended).toHaveLength(1)
    const change = appended[0]!
    expect(String(change.scope)).toBe(String(personalScope('u-1')))
    expect(String(change.entityType)).toBe(SYNC_DEBUG_ECHO_ENTITY_TYPE)
    expect(String(change.entityId)).toBe('e-42')
    expect(change.op).toBe('upsert')
    expect(change.mutationRef).toBe('mutation:cg-1:c-1:7')
    expect(change.postImage).toEqual({
      echo: 'khala hears you',
      entityId: 'e-42',
      scope: 'scope.user.u-1',
    })
  })
})
