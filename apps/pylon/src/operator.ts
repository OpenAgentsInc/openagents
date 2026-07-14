import type { PylonHostInventoryProjection } from './inventory.js'
import { assertPublicProjectionSafe } from './state.js'

export type OperatorMode = 'automated' | 'inspect' | 'recovery'

export type PylonOperatorSnapshot = {
  schema: 'openagents.pylon.operator_snapshot.v0.4'
  desiredMode: OperatorMode
  intakeState: 'automatic' | 'paused' | 'blocked'
  recentJobRefs: string[]
  receiptRefs: string[]
  blockerRefs: string[]
  paymentCapability: {
    state: 'retired'
    mutationAllowed: false
    paidCapacityFallbackAllowed: false
    reasonRef: 'reason.public.pylon.money_capability_retired.v1'
  }
  inspect: {
    inventoryFreshness: string
    eligibleInventoryCount: number
    backendRefs: string[]
    resourceMode: string
    blockerRefs: string[]
  }
  recovery: {
    headlessCommandRefs: string[]
    operatorOptInRequired: boolean
    sandboxProfileRequired: boolean
    budgetRequired: boolean
  }
}

export function createOperatorSnapshot(input: {
  inventory: PylonHostInventoryProjection
  recentJobRefs?: string[]
  receiptRefs?: string[]
  desiredMode?: OperatorMode
}) {
  const blockerRefs = [...new Set(input.inventory.blockerRefs)]
  const snapshot: PylonOperatorSnapshot = {
    schema: 'openagents.pylon.operator_snapshot.v0.4',
    desiredMode: input.desiredMode ?? 'automated',
    intakeState: blockerRefs.length > 0 ? 'blocked' : 'automatic',
    recentJobRefs: input.recentJobRefs ?? [],
    receiptRefs: input.receiptRefs ?? [],
    blockerRefs,
    paymentCapability: {
      state: 'retired',
      mutationAllowed: false,
      paidCapacityFallbackAllowed: false,
      reasonRef: 'reason.public.pylon.money_capability_retired.v1',
    },
    inspect: {
      inventoryFreshness: input.inventory.freshness,
      eligibleInventoryCount: input.inventory.eligibleInventoryCount,
      backendRefs: input.inventory.backendHealth.map((backend) => `${backend.backendRef}.${backend.state}`),
      resourceMode: input.inventory.resourceMode,
      blockerRefs: input.inventory.blockerRefs,
    },
    recovery: {
      headlessCommandRefs: [
        'command.pylon.status_json',
        'command.pylon.inventory_json',
        'command.pylon.assignment_poll',
      ],
      operatorOptInRequired: true,
      sandboxProfileRequired: true,
      budgetRequired: true,
    },
  }
  assertPublicProjectionSafe(snapshot)
  return snapshot
}

export function formatOperatorSnapshotText(snapshot: PylonOperatorSnapshot) {
  const backendRefs = snapshot.inspect.backendRefs.slice(0, 4).join('\n ')
  const blockers = snapshot.blockerRefs.length > 0 ? snapshot.blockerRefs.slice(0, 4).join('\n ') : 'none'

  return [
    `Operate: ${snapshot.desiredMode}`,
    `Intake: ${snapshot.intakeState}`,
    `Jobs: ${snapshot.recentJobRefs.length}`,
    `Payments: ${snapshot.paymentCapability.state}`,
    `Paid capacity fallback: denied`,
    `Receipts: ${snapshot.receiptRefs.length}`,
    '',
    `Inspect: ${snapshot.inspect.inventoryFreshness}`,
    `Eligible: ${snapshot.inspect.eligibleInventoryCount}`,
    backendRefs ? ` ${backendRefs}` : ' backends: none',
    '',
    `Recovery: opt-in gates`,
    ` ${snapshot.recovery.headlessCommandRefs.join('\n ')}`,
    '',
    `Blockers: ${blockers}`,
  ].join('\n')
}
