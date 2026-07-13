import {
  PortableCapabilityBroker,
  type CapabilityBrokerConfig,
} from "@openagentsinc/portable-session-contract"

import {
  PostgresPortableCapabilityBrokerStore,
  type PortableCapabilityBrokerStoreScope,
} from "./portable-capability-broker-store.js"
import {
  PortableSessionMoveCoordinator,
  PortableSessionMoveError,
  type PortableSessionMoveInput,
  type PortableSessionMoveResult,
} from "./portable-session-move.js"
import type { SyncTransactionWriter } from "./outbox-writer.js"
import type { SyncSql } from "./sql.js"

export type PortableSessionMoveRuntimeBrokerConfig = Omit<
  CapabilityBrokerConfig,
  "atomicStateStore" | "stateStore" | "evidenceSink"
>

export type PortableSessionMoveRuntimeInput = Readonly<{
  moveRef: string
  move: PortableSessionMoveInput
  broker: PortableSessionMoveRuntimeBrokerConfig
}>

export type PortableSessionMoveRuntimeConfig = Readonly<{
  sql: SyncSql
  transaction: <A>(run: (writer: SyncTransactionWriter) => Promise<A>) => Promise<A>
  /** Test seam only; production omits it and always uses the real coordinator. */
  coordinatorFactory?: (
    broker: PortableCapabilityBroker,
  ) => Pick<PortableSessionMoveCoordinator, "move">
}>

const isTerminal = (status: PortableSessionMoveResult["status"]): boolean =>
  status === "completed" || status === "replayed" || status === "failed"

/**
 * Owner-side PORT-03 production composition.
 *
 * The exact move claim is acquired before broker restoration, remains held
 * across every refs-only broker CAS commit, and is deliberately retained when
 * authority or activation needs reconciliation. A process restart with the
 * same bytes reacquires the same claim and restores the broker; a conflicting
 * move fails before any target or vault effect can run.
 */
export class PostgresPortableSessionMoveRuntime {
  constructor(private readonly config: PortableSessionMoveRuntimeConfig) {}

  async move(input: PortableSessionMoveRuntimeInput): Promise<PortableSessionMoveResult> {
    if (input.move.command.destinationTargetRef === undefined ||
        input.move.command.destinationTargetRef !== input.move.destination.targetRef ||
        input.move.command.checkpointRef === undefined ||
        !["attach", "move", "failback"].includes(input.move.command.kind)) {
      throw new PortableSessionMoveError(
        "target_mismatch",
        "portable move runtime input is incomplete or target-mismatched",
      )
    }
    const scope = this.scope(input)
    const store = new PostgresPortableCapabilityBrokerStore(this.config.sql, scope)
    const revision = await store.readRevision()
    await store.acquireMoveClaim(revision)

    const broker = await PortableCapabilityBroker.restore({
      ...input.broker,
      atomicStateStore: store,
    })
    const coordinator = this.config.coordinatorFactory?.(broker) ??
      new PortableSessionMoveCoordinator({
        sql: this.config.sql,
        transaction: this.config.transaction,
        broker,
      })

    const result = await coordinator.move(input.move)
    if (isTerminal(result.status)) {
      await store.releaseMoveClaim(store.currentRevision())
    }
    return result
  }

  private scope(input: PortableSessionMoveRuntimeInput): PortableCapabilityBrokerStoreScope {
    return {
      ownerRef: input.move.command.ownerRef,
      sessionRef: input.move.command.sessionRef,
      moveClaim: {
        moveRef: input.moveRef,
        commandRef: input.move.command.commandRef,
        sourceAttachmentRef: input.move.command.expectedAttachmentRef,
        sourceGeneration: input.move.command.expectedGeneration,
        destinationTargetRef: input.move.command.destinationTargetRef!,
      },
    }
  }
}
