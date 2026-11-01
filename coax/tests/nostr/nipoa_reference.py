#!/usr/bin/env python3
"""Independent NIP-OA and NIP-AA values for coax/lib/nostr/nipoa.coax,
nipaa.coax and nostr/examples/checks/agent-auth.coax.

Uses the pure-Python BIP-340 signer and NIP-01 serializer in
nip59_reference.py (hashlib and integers only; it shares no code with Coax).
Tooling only, never shipped. Test keys only: the owner's secret is 1, the
agent's 2 and a second owner's 3, as in NIP-OA's test vectors.

  python3 coax/tests/nostr/nipoa_reference.py

It checks NIP-OA's published test vector and signed event example (the
hash, both signatures, the event id and the tag bytes), then prints the
values the Coax checks compare: the owner's signature with 32 zero aux
bytes and the agent's AUTH event for chapter 87's relay and challenge.
"""
import hashlib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import nip59_reference as r  # noqa: E402

CONDS = "kind=1&created_at<1713957000"
NIP_HASH = "08cdecd55af4c28d3801fd69615dcf5cc04fab3bc134b38a840bf157197069a6"
NIP_SIG = ("8b7df2575caf0a108374f8471722b233c53f9ff827a8b0f91861966c3b9dd5cb"
           "2e189eae9f49d72187674c2f5bd244145e10ff86c9f257ffe65a1ee5f108b369")
NIP_EVENT_ID = "d892a65e7677e0554ebb70ee16deeb6a0727dba46450fb4bc001291d7bff971b"
NIP_EVENT_SIG = ("7fd38992b70b5e9e113644e51b4c8ee2227f3bdd402b1855f8786c0600394ab3"
                 "ec2621742a7bad0b0000b93d4d1ae6e39525f286a3c1029f43f46c3359a6c76f")
NIP_TAG_HEX = (
    "5b2261757468222c2237396265363637656639646362626163353561303632393563653837306230373032396266636462326463"
    "653238643935396632383135623136663831373938222c226b696e643d3126637265617465645f61743c31373133393537303030"
    "222c2238623764663235373563616630613130383337346638343731373232623233336335336639666638323761386230663931"
    "3836313936366333623964643563623265313839656165396634396437323138373637346332663562643234343134356531306666"
    "3836633966323537666665363561316565356631303862333639225d")
ZERO = bytes(32)


def preimage(agent_hex, conditions):
    return ("nostr:agent-auth:" + agent_hex + ":" + conditions).encode()


def main():
    owner, agent, other = r.k(1), r.k(2), r.k(3)
    op, ap, xp = r.xonly(owner).hex(), r.xonly(agent).hex(), r.xonly(other).hex()
    pre = preimage(ap, CONDS)
    h = hashlib.sha256(pre).digest()
    tag = ["auth", op, CONDS, NIP_SIG]
    tags = [tag]
    eid = r.event_id(ap, 1713956400, 1, tags, "owner-attested agent event")
    checks = [
        ("preimage is 110 bytes", len(pre) == 110),
        ("sha256(preimage) is NIP-OA's", h.hex() == NIP_HASH),
        ("NIP-OA's auth_sig verifies under the owner", r.verify(bytes.fromhex(op), h, bytes.fromhex(NIP_SIG))),
        ("the tag bytes are NIP-OA's", r.dumps(tag).encode().hex() == NIP_TAG_HEX),
        ("the example event's id is NIP-OA's", eid.hex() == NIP_EVENT_ID),
        ("the example event's sig verifies under the agent", r.verify(bytes.fromhex(ap), eid, bytes.fromhex(NIP_EVENT_SIG))),
    ]
    for name, ok in checks:
        print(f"{name}: {ok}")
    print("owner:", op)
    print("agent:", ap)
    print("second owner:", xp)
    print("owner signature, aux 0:", r.sign(owner, h, ZERO).hex())
    hx = hashlib.sha256(preimage(ap, CONDS)).digest()
    print("second owner's signature, aux 0:", r.sign(other, hx, ZERO).hex())
    auth = [["relay", "wss://relay.example.com"], ["challenge", "challenge-87"], tag]
    print("AUTH event, aux 0:", r.signed(agent, ZERO, 1713956400, 22242, auth, ""))
    if not all(ok for _, ok in checks):
        sys.exit(1)


if __name__ == "__main__":
    main()
