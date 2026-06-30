import {
  type ForfeitLaborEscrowInput,
  type LaborEscrowResult,
  type ReleaseLaborEscrowInput,
  type ReserveLaborEscrowInput,
  forfeitLaborEscrow,
  releaseLaborEscrow,
  reserveLaborEscrow,
} from './labor-escrow'

export type BondSettlementAdapterKind = 'credit_ledger'

export type BondSettlementHoldInput = ReserveLaborEscrowInput
export type BondSettlementReleaseInput = ReleaseLaborEscrowInput
export type BondSettlementForfeitInput = ForfeitLaborEscrowInput

export type BondSettlementAdapter = Readonly<{
  kind: BondSettlementAdapterKind
  hold: (input: BondSettlementHoldInput) => Promise<LaborEscrowResult>
  release: (input: BondSettlementReleaseInput) => Promise<LaborEscrowResult>
  forfeit: (input: BondSettlementForfeitInput) => Promise<LaborEscrowResult>
}>

export const createCreditLedgerBondSettlementAdapter = (
  db: D1Database,
): BondSettlementAdapter => ({
  kind: 'credit_ledger',
  hold: input => reserveLaborEscrow(db, input),
  release: input => releaseLaborEscrow(db, input),
  forfeit: input => forfeitLaborEscrow(db, input),
})
