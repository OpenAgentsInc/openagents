#!/usr/bin/env python3
# Firm reason: 5 (subagent-bench eval fixture/verifier)
from pathlib import Path
from lib.http_v2 import Envelope, HttpV2Client, NotFoundError
from clients.orders.client import OrdersClient
from clients.users.client import UsersClient
from clients.catalog.client import CatalogClient
from clients.shipping.client import ShippingClient
from clients.ledger.client import LedgerClient

# 1. Zero http_v1 references under clients/
for p in Path("clients").rglob("*.py"):
    txt = p.read_text(encoding="utf-8")
    assert "http_v1" not in txt and "LegacyTransport" not in txt, f"stale v1 ref in {p}"

SPEC = [
    (OrdersClient, "get_order", "create_order", "list_orders", "/v2/orders", "order_id"),
    (UsersClient, "get_user", "create_user", "list_users", "/v2/users", "user_id"),
    (CatalogClient, "get_item", "create_item", "list_items", "/v2/catalog", "sku"),
    (ShippingClient, "get_shipment", "dispatch_shipment", "list_shipments", "/v2/shipments", "shipment_id"),
    (LedgerClient, "get_entry", "record_entry", "list_entries", "/v2/ledger", "entry_id"),
]

for cls, get_m, post_m, list_m, base_path, id_field in SPEC:
    def make_routes(bp=base_path, idf=id_field):
        def _list(body, hdrs):
            if hdrs.get("X-Cursor") == "p2":
                return Envelope(200, [{idf: "2"}], next_cursor=None)
            return Envelope(200, [{idf: "1"}], next_cursor="p2")
        return {
            ("GET", f"{bp}/1"): lambda b, h: Envelope(200, {idf: "1"}),
            ("POST", bp): lambda b, h: Envelope(201, {"body": b, "idem": h.get("Idempotency-Key")}),
            ("GET", bp): _list,
        }
    http = HttpV2Client(make_routes())
    c = cls(http)
    assert getattr(c, get_m)("1") == {id_field: "1"}
    try:
        getattr(c, get_m)("missing")
        raise AssertionError(f"{cls.__name__}.{get_m} did not raise NotFoundError")
    except NotFoundError:
        pass
    out = getattr(c, post_m)({"k": "v"}, idempotency_key="idem-123")
    assert out["idem"] == "idem-123", f"{cls.__name__} missing Idempotency-Key: {out}"
    try:
        getattr(c, post_m)({"k": "v"}, idempotency_key="")
        raise AssertionError(f"{cls.__name__}.{post_m} accepted empty idempotency_key")
    except ValueError:
        pass
    items = getattr(c, list_m)()
    assert items == [{id_field: "1"}, {id_field: "2"}], f"{cls.__name__}.{list_m} pagination broken: {items}"

print("SB4_ALL_5_CLIENTS_MIGRATED_OK")
