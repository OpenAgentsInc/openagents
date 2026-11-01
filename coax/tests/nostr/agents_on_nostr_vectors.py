#!/usr/bin/env python3
"""The event sequence of chapter 102, Agents on Nostr, computed independently.

Written from NIP-01, NIP-OA, NIP-42, NIP-44 v2, BIP-340, NIP-CJ and NIP-RUN
with only hashlib and integers (signing and encryption come from
nip59_reference.py, which shares no code with Coax). Tooling only, never
shipped. Test keys only: the owner is the secp256k1 scalar 1, the agent 2,
the buyer 3, the reviewer 4 and a stranger 7. BIP-340 aux is 32 zero bytes;
the NIP-44 nonces are 01..01, 02..02 and 03..03.

  python3 coax/tests/nostr/agents_on_nostr_vectors.py > coax/tests/nostr/agents_on_nostr_vectors.coax

It prints the Coax constants nostr/examples/checks/agents-on-nostr.coax
compares with the example: the five public keys, the owner's attestation
hash and signature, the AUTH event's id, the conversation key, the RUN
record digests, and the job, acceptance, result and impostor events' ids
and signatures.
"""
import hashlib
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from nip59_reference import conversation_key, dumps, event_id, nip44_encrypt, sign, xonly  # noqa: E402

T0 = 1790000000
OWNER, AGENT, BUYER, REVIEWER, STRANGER = ((n).to_bytes(32, "big") for n in (1, 2, 3, 4, 7))
O, A, B, R, X = (xonly(k).hex() for k in (OWNER, AGENT, BUYER, REVIEWER, STRANGER))
AUX = bytes(32)
RELAY = "wss://relay.example.com"
CHALLENGE = "agent-challenge-1"
CONDS = "created_at<1790086400"
RUN = "2" * 64
V = "openagents.execution.v1"


def run_digest(record):
    """NIP-RUN: SHA-256 of the schema line and the record's JCS bytes."""
    jcs = json.dumps(record, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return "sha256:" + hashlib.sha256(("openagents.run-record.v1\n" + jcs).encode()).hexdigest()


def record(seq, previous, typ, step, time, data):
    return {"run": RUN, "seq": seq, "previous": previous, "controller": A, "generation": 0, "type": typ,
            "subject": {"step": step, "iteration": None, "attempt": 1 if step else None}, "time": time, "data": data}


def event(secret, at, kind, tags, content):
    pub = xonly(secret).hex()
    eid = event_id(pub, at, kind, tags, content)
    return eid.hex(), sign(secret, eid, AUX).hex()


def main():
    out = []
    say = lambda name, value: out.append(f'PY_{name} :: "{value}";')
    for name, value in (("OWNER", O), ("AGENT", A), ("BUYER", B), ("REVIEWER", R), ("STRANGER", X)):
        say(name, value)

    # NIP-OA: the owner signs sha256("nostr:agent-auth:" + agent + ":" + conditions).
    attest = hashlib.sha256(f"nostr:agent-auth:{A}:{CONDS}".encode()).digest()
    attest_sig = sign(OWNER, attest, AUX).hex()
    say("ATTEST_HASH", attest.hex())
    say("ATTEST_SIG", attest_sig)
    auth_tags = [["relay", RELAY], ["challenge", CHALLENGE], ["auth", O, CONDS, attest_sig]]
    say("AUTH_ID", event(AGENT, T0, 22242, auth_tags, "")[0])

    ck = conversation_key(BUYER, bytes.fromhex(A))
    assert ck == conversation_key(AGENT, bytes.fromhex(B))
    say("CK", ck.hex())

    execute = dumps({"v": V, "requires": ["openagents.labor-binding.v1"], "type": "execute", "request": "req-1",
                     "attempt": 1, "run": RUN, "target": "op:summarize", "input": {"text": "one paragraph"},
                     "deadline": 1790003600, "retain_until": 1790086400})
    say("EXECUTE", execute.replace('"', '\\"'))
    payload = nip44_encrypt(ck, bytes([1]) * 32, execute.encode())
    req_id, req_sig = event(BUYER, T0 + 60, 25920, [["p", A], ["expiration", "1790003600"]], payload)
    say("REQUEST_ID", req_id)
    say("REQUEST_SIG", req_sig)

    digests = []
    for seq, typ, step, time, data in ((0, "created", None, T0 + 61, {}), (1, "admitted", None, T0 + 61, {}),
                                       (2, "dispatched", "execute", T0 + 62, {}),
                                       (3, "resolved", "execute", T0 + 90, {"outcome": "succeeded"}),
                                       (4, "settled", None, T0 + 141, {"paid_msat": 50000})):
        digests.append(run_digest(record(seq, digests[-1] if digests else None, typ, step, time, data)))
        say(f"RUN_{seq}", digests[-1])

    answer_tags = [["p", B], ["e", req_id]]
    accepted = dumps({"v": V, "requires": [], "type": "accepted", "request": "req-1", "attempt": 1, "run": RUN,
                      "record": digests[1], "controller": A, "retain_until": 1790086400})
    acc_id, acc_sig = event(AGENT, T0 + 62, 27020, answer_tags, nip44_encrypt(ck, bytes([2]) * 32, accepted.encode()))
    say("ACCEPTED_ID", acc_id)
    say("ACCEPTED_SIG", acc_sig)
    result = dumps({"v": V, "requires": [], "type": "result", "request": "req-1", "attempt": 1, "run": RUN,
                    "status": "succeeded", "output": {"summary": "one sentence"}, "record": digests[3]})
    result_payload = nip44_encrypt(ck, bytes([3]) * 32, result.encode())
    res_id, res_sig = event(AGENT, T0 + 92, 26920, answer_tags, result_payload)
    say("RESULT_ID", res_id)
    say("RESULT_SIG", res_sig)
    say("IMPOSTOR_ID", event(STRANGER, T0 + 92, 26920, answer_tags, result_payload)[0])

    # The Lightning proof: SHA-256 of the preimage 01 02 ... 20 is the payment hash.
    say("PAYMENT_HASH", hashlib.sha256(bytes(range(1, 33))).hexdigest())
    print("// Generated by coax/tests/nostr/agents_on_nostr_vectors.py. Do not edit.")
    print("\n".join(out))


if __name__ == "__main__":
    main()
