const KEY = "rejected_deposits_v1";
let cache: { txid: string; vout: number }[] | null = null;

export function getRejectedDeposits(): { txid: string; vout: number }[] {
  if (cache !== null) return cache;
  try {
    const raw = typeof localStorage !== "undefined" ? localStorage.getItem(KEY) : null;
    if (!raw) {
      cache = [];
      return cache;
    }
    const p = JSON.parse(raw);
    cache = Array.isArray(p) ? p : [];
    return cache;
  } catch {
    cache = [];
    return cache;
  }
}

export function isDepositRejected(txid: string, vout: number): boolean {
  return getRejectedDeposits().some((d) => d.txid === txid && d.vout === vout);
}

export function rejectDeposit(txid: string, vout: number): void {
  const list = getRejectedDeposits();
  if (list.some((d) => d.txid === txid && d.vout === vout)) return;
  list.push({ txid, vout });
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    // ignore
  }
  cache = list;
}
