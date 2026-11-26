# Firm reason: 5 (subagent-bench eval fixture/verifier)
from dataclasses import dataclass
from typing import Dict, List, Tuple


class InsufficientStockError(RuntimeError):
    pass


@dataclass
class StockLot:
    lot_id: str
    received_at: int
    quantity: int


@dataclass
class Reservation:
    res_id: str
    sku: str
    allocations: List[Tuple[str, int]]
    expires_at: int


class WarehouseAllocator:
    def __init__(self):
        self._lots: Dict[str, List[StockLot]] = {}
        self._reservations: Dict[str, Reservation] = {}

    def add_lot(self, sku: str, lot_id: str, quantity: int, received_at: int) -> None:
        if quantity <= 0:
            raise ValueError("quantity must be positive")
        lots = self._lots.setdefault(sku, [])
        lots.append(StockLot(lot_id=lot_id, received_at=received_at, quantity=quantity))
        lots.sort(key=lambda l: (l.received_at, l.lot_id))

    def purge_expired(self, now_s: int) -> int:
        expired = [rid for rid, r in self._reservations.items() if r.expires_at <= now_s]
        for rid in expired:
            res = self._reservations.pop(rid)
            lot_map = {l.lot_id: l for l in self._lots.get(res.sku, [])}
            for lid, qty in res.allocations:
                lot_map[lid].quantity += qty
        return len(expired)

    def available(self, sku: str, now_s: int) -> int:
        # BUG 1: does not purge expired reservations before reporting available quantity!
        return sum(l.quantity for l in self._lots.get(sku, []))

    def reserve_fifo(self, res_id: str, sku: str, quantity: int, now_s: int, ttl_s: int) -> List[Tuple[str, int]]:
        if quantity <= 0 or ttl_s <= 0:
            raise ValueError("invalid quantity or ttl")
        if res_id in self._reservations:
            raise ValueError("duplicate reservation id")
        # BUG 1 also affects reserve_fifo: doesn't purge expired first
        lots = self._lots.get(sku, [])
        need = quantity
        picks: List[Tuple[str, int]] = []
        # BUG 2: mutates lot.quantity in place before confirming total availability (no atomic rollback on shortfall!)
        for lot in lots:
            if need == 0:
                break
            take = min(lot.quantity, need)
            if take > 0:
                lot.quantity -= take
                picks.append((lot.lot_id, take))
                need -= take
        if need > 0:
            raise InsufficientStockError(f"short {need} units of {sku}")
        self._reservations[res_id] = Reservation(res_id=res_id, sku=sku, allocations=picks, expires_at=now_s + ttl_s)
        return picks
