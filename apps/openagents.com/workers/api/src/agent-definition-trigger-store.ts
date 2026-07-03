import {
  AgentDefinitionTrigger,
  AgentDefinitionTriggerRecordSchemaLiteral,
  decodeAgentDefinitionTriggerRecord,
  type AgentDefinition,
  type AgentDefinitionTriggerRecord,
  type AgentDefinitionTriggerState,
} from '@openagentsinc/agent-runtime-schema'

import { computeNextCronRunAt } from './agent-definition-cron'
import { parseJsonWithSchema } from './json-boundary'

type AgentDefinitionTriggerRow = Readonly<{
  trigger_id: string
  owner_ref: string
  definition_id: string
  trigger_ref: string
  trigger_json: string
  state: AgentDefinitionTriggerState
  consecutive_failures: number
  next_run_at: string | null
  paused_at: string | null
  pause_reason: string | null
  created_at: string
  updated_at: string
}>

type D1ChangeResult = Readonly<{
  meta?: Readonly<{
    changes?: number
  }>
}>

export type AgentDefinitionTriggerStore = Readonly<{
  replaceDefinitionTriggers: (
    ownerAgentUserId: string,
    definition: AgentDefinition,
    nowIso: string,
  ) => Promise<ReadonlyArray<AgentDefinitionTriggerRecord>>
  listDefinitionTriggers: (
    ownerAgentUserId: string,
    definitionId: string,
  ) => Promise<ReadonlyArray<AgentDefinitionTriggerRecord>>
  pauseTrigger: (
    ownerAgentUserId: string,
    triggerRef: string,
    pausedAt: string,
    reason: string,
  ) => Promise<boolean>
  enableTrigger: (
    ownerAgentUserId: string,
    triggerRef: string,
    updatedAt: string,
  ) => Promise<boolean>
  recordTriggerFailure: (
    ownerAgentUserId: string,
    triggerRef: string,
    updatedAt: string,
  ) => Promise<boolean>
  recordTriggerSuccess: (
    ownerAgentUserId: string,
    triggerRef: string,
    nextRunAt: string | undefined,
    updatedAt: string,
  ) => Promise<boolean>
}>

const triggerIdFor = (
  definition: AgentDefinition,
  triggerRef: string,
): string => `${definition.id}:${triggerRef}`

const nextRunAtForTrigger = (
  trigger: AgentDefinition['triggers'][number],
  nowIso: string,
): string | undefined =>
  trigger.kind === 'cron'
    ? computeNextCronRunAt({
        afterIso: nowIso,
        expr: trigger.expr,
        tz: trigger.tz,
      })
    : undefined

const rowToTriggerRecord = (
  row: AgentDefinitionTriggerRow,
): AgentDefinitionTriggerRecord =>
  decodeAgentDefinitionTriggerRecord({
    schema: AgentDefinitionTriggerRecordSchemaLiteral,
    triggerId: row.trigger_id,
    ownerRef: row.owner_ref,
    definitionId: row.definition_id,
    triggerRef: row.trigger_ref,
    trigger: parseJsonWithSchema(AgentDefinitionTrigger, row.trigger_json),
    state: row.state,
    consecutiveFailures: row.consecutive_failures,
    ...(row.next_run_at === null ? {} : { nextRunAt: row.next_run_at }),
    ...(row.paused_at === null ? {} : { pausedAt: row.paused_at }),
    ...(row.pause_reason === null ? {} : { pauseReason: row.pause_reason }),
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  })

const changed = (result: D1ChangeResult): boolean =>
  (result.meta?.changes ?? 0) > 0

export const makeD1AgentDefinitionTriggerStore = (
  db: D1Database,
): AgentDefinitionTriggerStore => {
  const listDefinitionTriggers = async (
    ownerAgentUserId: string,
    definitionId: string,
  ): Promise<ReadonlyArray<AgentDefinitionTriggerRecord>> => {
    const rows = await db
      .prepare(
        `SELECT trigger_id, owner_ref, definition_id, trigger_ref, trigger_json,
                state, consecutive_failures, next_run_at, paused_at,
                pause_reason, created_at, updated_at
           FROM agent_definition_triggers
          WHERE owner_agent_user_id = ?
            AND definition_id = ?
          ORDER BY trigger_ref ASC`,
      )
      .bind(ownerAgentUserId, definitionId)
      .all<AgentDefinitionTriggerRow>()

    return (rows.results ?? []).map(rowToTriggerRecord)
  }

  return {
    replaceDefinitionTriggers: async (
      ownerAgentUserId,
      definition,
      nowIso,
    ) => {
      if (definition.triggers.length === 0) {
        await db
          .prepare(
            `DELETE FROM agent_definition_triggers
              WHERE owner_agent_user_id = ?
                AND definition_id = ?`,
          )
          .bind(ownerAgentUserId, definition.id)
          .run()
      } else {
        const placeholders = definition.triggers.map(() => '?').join(', ')
        await db
          .prepare(
            `DELETE FROM agent_definition_triggers
              WHERE owner_agent_user_id = ?
                AND definition_id = ?
                AND trigger_ref NOT IN (${placeholders})`,
          )
          .bind(
            ownerAgentUserId,
            definition.id,
            ...definition.triggers.map(trigger => trigger.triggerRef),
          )
          .run()
      }

      for (const trigger of definition.triggers) {
        const nextRunAt = nextRunAtForTrigger(trigger, nowIso)

        await db
          .prepare(
            `INSERT INTO agent_definition_triggers
              (trigger_id, owner_agent_user_id, owner_ref, definition_id,
               trigger_ref, trigger_kind, trigger_json, state,
               consecutive_failures, next_run_at, paused_at, pause_reason,
               created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, 'enabled', 0, ?, NULL, NULL, ?, ?)
             ON CONFLICT(owner_agent_user_id, trigger_ref) DO UPDATE SET
               trigger_id = excluded.trigger_id,
               owner_ref = excluded.owner_ref,
               definition_id = excluded.definition_id,
               trigger_kind = excluded.trigger_kind,
               trigger_json = excluded.trigger_json,
               next_run_at = excluded.next_run_at,
               updated_at = excluded.updated_at`,
          )
          .bind(
            triggerIdFor(definition, trigger.triggerRef),
            ownerAgentUserId,
            definition.ownerRef,
            definition.id,
            trigger.triggerRef,
            trigger.kind,
            JSON.stringify(trigger),
            nextRunAt ?? null,
            nowIso,
            nowIso,
          )
          .run()
      }

      return listDefinitionTriggers(ownerAgentUserId, definition.id)
    },
    listDefinitionTriggers,
    pauseTrigger: async (ownerAgentUserId, triggerRef, pausedAt, reason) => {
      const result = await db
        .prepare(
          `UPDATE agent_definition_triggers
              SET state = 'paused',
                  paused_at = ?,
                  pause_reason = ?,
                  updated_at = ?
            WHERE owner_agent_user_id = ?
              AND trigger_ref = ?`,
        )
        .bind(pausedAt, reason, pausedAt, ownerAgentUserId, triggerRef)
        .run()

      return changed(result)
    },
    enableTrigger: async (ownerAgentUserId, triggerRef, updatedAt) => {
      const result = await db
        .prepare(
          `UPDATE agent_definition_triggers
              SET state = 'enabled',
                  paused_at = NULL,
                  pause_reason = NULL,
                  updated_at = ?
            WHERE owner_agent_user_id = ?
              AND trigger_ref = ?`,
        )
        .bind(updatedAt, ownerAgentUserId, triggerRef)
        .run()

      return changed(result)
    },
    recordTriggerFailure: async (ownerAgentUserId, triggerRef, updatedAt) => {
      const result = await db
        .prepare(
          `UPDATE agent_definition_triggers
              SET consecutive_failures = consecutive_failures + 1,
                  updated_at = ?
            WHERE owner_agent_user_id = ?
              AND trigger_ref = ?`,
        )
        .bind(updatedAt, ownerAgentUserId, triggerRef)
        .run()

      return changed(result)
    },
    recordTriggerSuccess: async (
      ownerAgentUserId,
      triggerRef,
      nextRunAt,
      updatedAt,
    ) => {
      const result = await db
        .prepare(
          `UPDATE agent_definition_triggers
              SET consecutive_failures = 0,
                  next_run_at = ?,
                  updated_at = ?
            WHERE owner_agent_user_id = ?
              AND trigger_ref = ?`,
        )
        .bind(nextRunAt ?? null, updatedAt, ownerAgentUserId, triggerRef)
        .run()

      return changed(result)
    },
  }
}
