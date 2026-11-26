# Firm reason: 5 (subagent-bench eval fixture/verifier)
import sqlite3
from typing import Any, Dict, List


def init_db(conn: sqlite3.Connection) -> None:
    conn.execute("CREATE TABLE IF NOT EXISTS invoices (id TEXT PRIMARY KEY, owner_id TEXT, amount INT, status TEXT)")
    conn.commit()


def list_invoices_by_status(conn: sqlite3.Connection, status_filter: str) -> List[Dict[str, Any]]:
    # VULN: CWE-89 SQL injection via string formatting
    sql = f"SELECT id, owner_id, amount, status FROM invoices WHERE status = '{status_filter}' ORDER BY id"
    rows = conn.execute(sql).fetchall()
    return [{"id": r[0], "owner_id": r[1], "amount": r[2], "status": r[3]} for r in rows]


def get_invoice_detail(conn: sqlite3.Connection, invoice_id: str, requester_user_id: str) -> Dict[str, Any]:
    # VULN: CWE-639 IDOR - ignores requester_user_id ownership check!
    row = conn.execute("SELECT id, owner_id, amount, status FROM invoices WHERE id = ?", (invoice_id,)).fetchone()
    if not row:
        return {"error": "not_found"}
    return {"id": row[0], "owner_id": row[1], "amount": row[2], "status": row[3], "viewer": requester_user_id}


def compute_fee(amount_cents: int) -> int:
    return (amount_cents * 29 + 500) // 1000 + 30
