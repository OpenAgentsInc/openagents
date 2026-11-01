#!/usr/bin/env python3
"""An independent NIP-17 for nostr/examples/checks/private-messages.coax.

Written from NIP-17 on top of nip59_reference.py (NIP-01, NIP-44 v2, NIP-59,
BIP-340 and RFC 8439 with only hashlib, hmac, base64 and integers); it shares
no code with Coax. Tooling only, never shipped. Test keys only.

  python3 coax/tests/nostr/nip17_reference.py

It builds chapter 60's three messages from the same keys, nonces, aux bytes
and time offsets as nostr/examples/private-messages.coax: Alice to Bob and
Carol (three wraps), Bob's reply to the room (three wraps) and Alice to Bob
alone (two wraps). It prints every rumor id, wrap id and room id, opens each
wrap as its addressee, then opens the two wraps printed in NIP-17's example
with the keys NIP-17 publishes as nsec strings, decoded here with bech32.
"""
import hashlib
import json
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import nip59_reference as r  # noqa: E402

ALICE = r.k(3)
BOB = bytes.fromhex("b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef")
CAROL = r.k(7)
NOW = 1793300000
MESSAGE = "Are you going to the party tonight?"
REPLY = "Yes, at nine."
DIRECT = "Bring the speaker."


def fresh(m, i):
    """Message m's stand-in entropy for wrap i, as fixed_entropy in the example."""
    b = (16 * (m * 4 + i + 1)) % 256
    return {"wrap_key": r.k(11 + m * 4 + i),
            "seal_nonce": bytes([b + 1]) * 32, "wrap_nonce": bytes([b + 2]) * 32,
            "seal_aux": bytes([b + 3]) * 32, "wrap_aux": bytes([b + 4]) * 32,
            "seal_back": 3600 * (i + 1) + 600 * m, "wrap_back": 90000 + 7000 * i + 1000 * m}


def wrap_one(sender, to_pub, at, kind, tags, content, f):
    spub = r.xonly(sender).hex()
    rid = r.event_id(spub, at, kind, tags, content)
    rumor = r.dumps({"id": rid.hex(), "pubkey": spub, "created_at": at, "kind": kind, "tags": tags, "content": content})
    seal_at = at - f["seal_back"] % 172800
    wrap_at = at - f["wrap_back"] % 172800
    seal = r.signed(sender, f["seal_aux"], seal_at, 13, [],
                    r.nip44_encrypt(r.conversation_key(sender, to_pub), f["seal_nonce"], rumor.encode()))
    wrap = r.signed(f["wrap_key"], f["wrap_aux"], wrap_at, 1059, [["p", to_pub.hex()]],
                    r.nip44_encrypt(r.conversation_key(f["wrap_key"], to_pub), f["wrap_nonce"], seal.encode()))
    return rid.hex(), wrap


def send(m, sender, recipients, at, tags, content):
    """NIP-17: p tags for the recipients, then one wrap each and one for the sender."""
    rtags = [["p", r.xonly(x).hex()] for x in recipients] + tags
    out = []
    for i, who in enumerate(recipients + [sender]):
        rid, wrap = wrap_one(sender, r.xonly(who), at, 14, rtags, content, fresh(m, i))
        out.append((who, rid, wrap))
    return out


def room(pubs_hex):
    return hashlib.sha256(b"".join(sorted(bytes.fromhex(p) for p in set(pubs_hex)))).hexdigest()


def open_message(wrap_text, me):
    seal_text, rumor_text = r.unwrap(json.loads(wrap_text), me)
    rumor = json.loads(rumor_text)
    members = [rumor["pubkey"]] + [t[1] for t in rumor["tags"] if t[0] == "p"]
    assert r.xonly(me).hex() in members and rumor["kind"] in (14, 15)
    return json.loads(seal_text), rumor, room(members)


def chapter():
    names = {ALICE: "alice", BOB: "bob", CAROL: "carol"}
    for key in (ALICE, BOB, CAROL):
        print(names[key], r.xonly(key).hex())
    messages = [
        send(0, ALICE, [BOB, CAROL], NOW, [["subject", "Party"]], MESSAGE),
        None, None]
    first_id = messages[0][0][1]
    messages[1] = send(1, BOB, [ALICE, CAROL], NOW + 60, [["e", first_id]], REPLY)
    messages[2] = send(2, ALICE, [BOB], NOW + 120, [], DIRECT)
    for m, wraps in enumerate(messages):
        print("message", m, "rumor id", wraps[0][1])
        for who, rid, wrap in wraps:
            w = json.loads(wrap)
            seal, rumor, rid_room = open_message(wrap, who)
            assert rumor["id"] == rid
            print(" wrap to", names[who], "id", w["id"], "bytes", len(wrap), "created_at", w["created_at"], "room", rid_room)
            if m == 0 and who == BOB:
                print(" first wrap", wrap)
    print("rooms differ", messages[0][0][2] != messages[2][0][2])


CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"


def bech32_secret(text):
    hrp, data = text.rsplit("1", 1)
    values = [CHARSET.index(c) for c in data]
    poly = 1
    for v in [ord(c) >> 5 for c in hrp] + [0] + [ord(c) & 31 for c in hrp] + values:
        top = poly >> 25
        poly = (poly & 0x1FFFFFF) << 5 ^ v
        for i, g in enumerate((0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233DD, 0x2A1462B3)):
            if top >> i & 1:
                poly ^= g
    assert hrp == "nsec" and poly == 1
    acc, bits, out = 0, 0, bytearray()
    for v in values[:-6]:
        acc, bits = acc << 5 | v, bits + 5
        while bits >= 8:
            bits -= 8
            out.append(acc >> bits & 255)
    return bytes(out)


def nip17_example(path):
    """Open the two wraps printed in NIP-17's example with its published keys."""
    text = path.read_text()
    sender = bech32_secret("nsec1w8udu59ydjvedgs3yv5qccshcj8k05fh3l60k9x57asjrqdpa00qkmr89m")
    receiver = bech32_secret("nsec12ywtkplvyq5t6twdqwwygavp5lm4fhuang89c943nf2z92eez43szvn4dt")
    print("nip-17 sender", sender.hex(), r.xonly(sender).hex())
    print("nip-17 receiver", receiver.hex(), r.xonly(receiver).hex())
    blocks = re.findall(r"```json\n(\{\n   \"id\".*?)```", text, re.S)
    assert len(blocks) == 2
    for block, me in zip(blocks, (receiver, sender)):
        w = json.loads(block)
        seal, rumor, rid_room = open_message(block, me)
        print("nip-17 wrap", w["id"], "opens; seal created_at", seal["created_at"], "rumor", r.dumps(rumor), "room", rid_room)


if __name__ == "__main__":
    chapter()
    nip17_example(pathlib.Path(__file__).resolve().parents[3] / "nostr/nips/official/17.md")
