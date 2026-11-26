#!/usr/bin/env python3
# Firm reason: 5 (subagent-bench eval fixture/verifier)
import tempfile
from pathlib import Path
from engines.mini_sql import MiniDB
from engines.expr_vm import compile_expr, eval_expr, run_vm
from engines.diff_engine import apply_unified_patch, diff_ops, make_unified_patch
from engines.kv_wal import WalKV

# 1. mini_sql
db = MiniDB()
db.create_table("users", [
    {"id": 1, "name": "Alice", "dept": "eng"},
    {"id": 2, "name": "Bob", "dept": "eng"},
    {"id": 3, "name": "Cara", "dept": "sales"},
])
db.create_table("orders", [
    {"oid": 10, "uid": 1, "amt": 100},
    {"oid": 11, "uid": 1, "amt": 50},
    {"oid": 12, "uid": 2, "amt": 200},
    {"oid": 13, "uid": 3, "amt": 75},
])
r1 = db.query("SELECT dept, COUNT(*) AS n, SUM(amt) AS total FROM users INNER JOIN orders ON users.id = orders.uid WHERE amt >= 50 GROUP BY dept ORDER BY total DESC")
assert len(r1) == 2 and r1[0]["dept"] == "eng" and r1[0]["n"] == 3 and r1[0]["total"] == 350, r1
assert r1[1]["dept"] == "sales" and r1[1]["total"] == 75, r1
r2 = db.query("SELECT name FROM users WHERE dept = 'eng' ORDER BY name ASC LIMIT 1 OFFSET 1")
assert r2 == [{"name": "Bob"}], r2

# 2. expr_vm
bc = compile_expr("let x = 2 ** 3 ** 2 in if x > 100 and (false or true) then clamp(x - 500, 0, 20) else 0")
assert isinstance(bc, list) and len(bc) > 0
assert run_vm(bc) == 12
env = {"a": 10}
assert eval_expr("let a = 3 in a * 4 + max(1, 5, 2)", env) == 17
assert env == {"a": 10}, "caller env mutated!"
# short-circuit check: division by zero must NOT trigger when skipped
assert eval_expr("false and (1 / 0 == 1)") is False
assert eval_expr("true or (1 / 0 == 1)") is True

# 3. diff_engine
old = "\n".join([f"line {i}" for i in range(1, 21)]) + "\n"
new_lines = [f"line {i}" for i in range(1, 21)]
new_lines[2] = "line 3 MODIFIED"
new_lines.insert(10, "inserted A")
new_lines.pop(16)
new = "\n".join(new_lines) + "\n"
ops = diff_ops(old.splitlines(), new.splitlines())
assert any(k == "-" and v == "line 3" for k, v in ops)
assert any(k == "+" and v == "inserted A" for k, v in ops)
patch = make_unified_patch(old, new, "old.txt", "new.txt", context=2)
assert "@@" in patch and "--- old.txt" in patch and "+++ new.txt" in patch
assert apply_unified_patch(old, patch) == new
try:
    apply_unified_patch("completely different\n", patch)
    raise AssertionError("corrupt patch context should fail")
except ValueError:
    pass

# 4. kv_wal
with tempfile.TemporaryDirectory() as td:
    p = Path(td) / "store.wal"
    kv = WalKV(p)
    kv.put(b"user:1", b"alice")
    kv.put(b"user:2", b"bob")
    kv.put(b"user:1", b"alice_v2")
    kv.delete(b"user:2")
    kv.put(b"cfg:mode", b"fast")
    kv.close()
    # Corrupt tail (simulate crash mid-write)
    with open(p, "ab") as fh:
        fh.write(b"WAL1\x01\x00\x00\x00\x05broken")
    kv2 = WalKV(p)
    assert kv2.get(b"user:1") == b"alice_v2"
    assert kv2.get(b"user:2") is None
    assert kv2.keys(b"user:") == [b"user:1"]
    before_sz = p.stat().st_size
    live = kv2.compact()
    assert live == 2
    assert p.stat().st_size < before_sz
    kv2.close()
    kv3 = WalKV(p)
    assert kv3.get(b"user:1") == b"alice_v2" and kv3.get(b"cfg:mode") == b"fast"
    kv3.close()

print("SB5_ALL_4_ENGINES_OK")
