import type { Payment, DepositInfo } from "@breeztech/breez-sdk-spark";
import { isDepositRejected } from "./depositState";

export interface ExtendedPayment extends Payment {
  isUnclaimedDeposit?: boolean;
  depositInfo?: DepositInfo;
}

export function convertDepositsToPayments(deposits: DepositInfo[]): ExtendedPayment[] {
  return deposits
    .filter((d) => !isDepositRejected(d.txid, d.vout))
    .map((d) => ({
      id: `deposit-${d.txid}-${d.vout}`,
      paymentType: "receive" as const,
      method: "deposit" as const,
      amount: BigInt(d.amountSats),
      timestamp: Math.floor(Date.now() / 1000),
      status: "pending" as const,
      fees: BigInt(0),
      isUnclaimedDeposit: true,
      depositInfo: d,
      details: { type: "deposit" as const, txId: d.txid },
    })) as ExtendedPayment[];
}

export function mergeDepositsWithTransactions(
  transactions: Payment[],
  deposits: DepositInfo[]
): ExtendedPayment[] {
  const depositPayments = convertDepositsToPayments(deposits);
  const extended = transactions.map((t) => ({ ...t, isUnclaimedDeposit: false })) as ExtendedPayment[];
  return [...depositPayments, ...extended];
}

export function isUnclaimedDepositPayment(
  payment: Payment | ExtendedPayment
): payment is ExtendedPayment {
  return (payment as ExtendedPayment).isUnclaimedDeposit === true;
}
