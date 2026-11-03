#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a separate Python model of BOLT 2's peer protocol decides each line outside Coax)
"""The cross-check of Bitcoin chapter 109. It computes every line the
chapter's example prints and compares the transcript with the fixture:

  python3 -I coax/tests/lightning/channel.py                      # compare with the fixture
  python3 -I coax/tests/lightning/channel.py --coax coax/bin/coax   # also run the example

Two Python nodes run the example's scenario. Their state follows BOLT 2's
rules with Eclair's change lists: each node's own updates are proposed,
signed, then acked; the peer's are proposed, acked, then signed; a
commitment is the previous one reduced by the owner's and the other
side's changes. That is a different design from channel.coax's numbered
stages, and it shares no code with Coax. Messages are written from BOLT
2's field lists; commitments, keys and signatures come from bolt3_tx.py,
keys.py and shachain.py; the force-close and closing transactions run
under chapter 36's Python transliteration of Bitcoin Core's interpreter.
BOLT 2 is pinned in coax/tests/vectors/upstream/bolts/02-peer-protocol.md;
its hash is checked first.
"""
import hashlib
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(1, str(HERE.parent / 'bitcoin'))
import bolts  # noqa: E402
import bolt3_tx as B  # noqa: E402
import keys as K  # noqa: E402
import shachain as SC  # noqa: E402
import script_reference as S  # noqa: E402
import tx_reference as T  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/lightning-channel.out'
EXAMPLE = 'bitcoin/examples/lightning-channel.coax'
RUN_STEPS = '250000000'
FLAGS = sum(S.F[f] for f in ('P2SH', 'STRICTENC', 'DERSIG', 'LOW_S', 'NULLDUMMY', 'MINIMALDATA', 'CLEANSTACK',
                             'CHECKLOCKTIMEVERIFY', 'CHECKSEQUENCEVERIFY', 'WITNESS', 'MINIMALIF', 'NULLFAIL',
                             'WITNESS_PUBKEYTYPE'))
CHAIN = bytes.fromhex('06226e46111a0b59caaf126043eb5bbf28c34f3a5e332a1fc7b2b73cf188910f')   # regtest
FUNDING_TXID = bytes.fromhex('8984484a580b825b9972d7adb15050b3ab624ccd731946b3eeddb92f4e7ef6be')  # hash order
FUNDING_SAT, PUSH_MSAT, FEERATE = 1000000, 200000000, 2500
NAMES = {32: 'open_channel', 33: 'accept_channel', 34: 'funding_created', 35: 'funding_signed', 36: 'channel_ready',
         38: 'shutdown', 39: 'closing_signed', 128: 'update_add_htlc', 130: 'update_fulfill_htlc',
         131: 'update_fail_htlc', 132: 'commitment_signed', 133: 'revoke_and_ack', 136: 'channel_reestablish'}


def u(v, n):
    return v.to_bytes(n, 'big')


def sha(b):
    return hashlib.sha256(b).digest()


def compact(key, digest):
    r, s = B.sign(key, digest)
    return u(r, 32) + u(s, 32)


def der(sig):
    return B.der(int.from_bytes(sig[:32], 'big'), int.from_bytes(sig[32:], 'big'))


def preimage(k):
    p = bytes((k + i) & 255 for i in range(32))
    return p, sha(p)


def runs(tx, spk, amount):
    t = T.unserialize(tx, True)
    i = t['ins'][0]
    return S.ERRORS[S.verify_script(i['script'], spk, i['witness'], FLAGS, S.Checker(t, 0, amount))]


# ---- specs: (owner's msat, other's msat, HTLCs), each HTLC (direction, id, msat, hash, expiry)

def reduce(spec, owner, other):
    """BOLT 2's commitment after the owner's and the other side's changes:
    adds first, then removals; a removal names the HTLC the remover
    received."""
    mine, theirs, htlcs = spec[0], spec[1], list(spec[2])
    for who, changes in (('out', owner), ('in', other)):
        for c in changes:
            if c[0] == 'add':
                if who == 'out':
                    mine -= c[2]
                else:
                    theirs -= c[2]
                htlcs.append((who, c[1], c[2], c[3], c[4]))
    for who, changes in (('in', owner), ('out', other)):      # the owner removes HTLCs it received
        for c in changes:
            if c[0] in ('fulfill', 'fail'):
                h = next(h for h in htlcs if h[0] == who and h[1] == c[1])
                htlcs.remove(h)
                to_owner = (c[0] == 'fulfill') == (who == 'in')
                if to_owner:
                    mine += h[2]
                else:
                    theirs += h[2]
    return mine, theirs, htlcs


class Node:
    def __init__(self, name, funder, seed, cfg):
        self.name, self.funder, self.cfg = name, funder, cfg
        s = {k: sha(seed + bytes([i])) for i, k in
             enumerate(('funding', 'revocation', 'payment', 'delayed', 'htlc', 'seed', 'temporary'), 1)}
        self.secret = s
        self.bp = {k: K.point(int.from_bytes(s[k], 'big')) for k in ('funding', 'revocation', 'payment', 'delayed', 'htlc')}
        self.out = []
        self.state = 'init'
        self.local_number = self.remote_number = 0
        self.waiting = False
        self.next_id = self.their_next_id = 0
        self.proposed, self.signed, self.acked = [], [], []                  # this node's changes
        self.their_proposed, self.their_acked, self.their_signed = [], [], []  # the peer's
        self.received_secrets = []
        self.shutdown_sent = self.shutdown_received = False
        self.close_sent = self.close_received = None
        self.closing = None
        self.temporary = s['temporary']

    def pcs(self, n):
        return SC.generate(self.secret['seed'], SC.MAX - n)

    def pcp(self, n):
        return K.point(int.from_bytes(self.pcs(n), 'big'))

    def send(self, m):
        self.out.append(m)

    # ---- commitments
    def view(self, local, n):
        """bolt3_tx's channel for this node's commitment n (local) or the peer's."""
        owner, other = (self, self.peer) if local else (self.peer, self)
        p = owner.pcp(n)
        opener, accepter = (self, self.peer) if self.funder else (self.peer, self)
        return B.Channel(FUNDING_TXID, 0, FUNDING_SAT, owner.bp['funding'], other.bp['funding'],
                         K.revocation_pubkey(other.bp['revocation'], p), K.pubkey(owner.bp['delayed'], p),
                         other.bp['payment'], K.pubkey(owner.bp['htlc'], p), K.pubkey(other.bp['htlc'], p),
                         other.cfg['delay'], owner.cfg['dust'], B.obscured(n, opener.bp['payment'], accepter.bp['payment']),
                         False, owner.funder)

    def outs(self, local, n, spec):
        htlcs = [{'offered': h[0] == 'out', 'amount_msat': h[2], 'expiry': h[4], 'payment_hash': h[3]} for h in spec[2]]
        return self.view(local, n).outputs(spec[0], spec[1], FEERATE, htlcs)

    def sigs(self, n, spec):
        """This node's signatures of the peer's commitment n: the commitment,
        then each HTLC output's transaction in output order."""
        ch = self.view(False, n)
        outs, _ = ch.outputs(spec[0], spec[1], FEERATE, [{'offered': h[0] == 'out', 'amount_msat': h[2],
                                                            'expiry': h[4], 'payment_hash': h[3]} for h in spec[2]])
        out = [compact(self.secret['funding'], ch.commitment_sighash(outs))]
        key = u(K.privkey(int.from_bytes(self.secret['htlc'], 'big'), self.peer.pcp(n)), 32)
        for i, o in enumerate(outs):
            if o[3] in ('offered', 'received'):
                (remote, _), _ = ch.htlc_sighashes(outs, i, FEERATE)
                out.append(compact(key, remote))
        return out

    # ---- messages
    def params(self, accept):
        c = self.cfg
        b = u(c['dust'], 8) + u(FUNDING_SAT * 1000, 8) + u(c['reserve'], 8) + u(c['htlc_min'], 8)
        b += u(3, 4) if accept else u(FEERATE, 4)
        b += u(c['delay'], 2) + u(c['max_accepted'], 2)
        b += b''.join(self.bp[k] for k in ('funding', 'revocation', 'payment', 'delayed', 'htlc')) + self.pcp(0)
        return b

    def open(self):
        self.send(u(32, 2) + CHAIN + self.temporary + u(FUNDING_SAT, 8) + u(PUSH_MSAT, 8) + self.params(False)
                  + b'\x00' + bytes.fromhex('01021000'))

    def fund(self):
        self.send(u(34, 2) + self.temporary + FUNDING_TXID + u(0, 2) + self.sigs(0, self.remote_spec)[0])

    def ready(self):
        self.send(u(36, 2) + FUNDING_TXID + self.pcp(1))

    def add(self, msat, payment_hash, expiry):
        c = ('add', self.next_id, msat, payment_hash, expiry)
        self.next_id += 1
        self.proposed.append(c)
        self.send(self.update(c))

    def update(self, c):
        if c[0] == 'add':
            return u(128, 2) + FUNDING_TXID + u(c[1], 8) + u(c[2], 8) + c[3] + u(c[4], 4) + bytes(1366)
        if c[0] == 'fulfill':
            return u(130, 2) + FUNDING_TXID + u(c[1], 8) + c[2]
        return u(131, 2) + FUNDING_TXID + u(c[1], 8) + u(0, 2)

    def remove(self, kind, hid, pre=None):
        c = (kind, hid, pre) if kind == 'fulfill' else (kind, hid)
        self.proposed.append(c)
        self.send(self.update(c))

    def commit_message(self):
        s = self.sigs(self.remote_number, self.remote_next)
        return u(132, 2) + FUNDING_TXID + s[0] + u(len(s) - 1, 2) + b''.join(s[1:])

    def commit(self):
        self.remote_next = reduce(self.remote_spec, self.their_acked, self.proposed)
        self.signed, self.proposed = self.proposed, []
        self.their_signed, self.their_acked = self.their_acked, []
        self.remote_number += 1
        self.waiting = True
        self.send(self.commit_message())

    def revoke_message(self):
        n = self.local_number
        return u(133, 2) + FUNDING_TXID + self.pcs(n - 1) + self.pcp(n + 1)

    def shutdown(self):
        self.shutdown_sent = True
        self.send(u(38, 2) + FUNDING_TXID + u(len(self.cfg['spk']), 2) + self.cfg['spk'])

    def reestablish(self):
        nr = self.remote_number - (1 if self.waiting else 0)
        last = self.received_secrets[-1] if nr else bytes(32)
        self.send(u(136, 2) + FUNDING_TXID + u(self.local_number + 1, 8) + u(nr, 8) + last + self.pcp(self.local_number))

    # ---- closing
    def quiet(self):
        return (self.shutdown_sent and self.shutdown_received and not self.waiting and not self.local_spec[2]
                and not self.remote_spec[2] and not (self.proposed or self.signed or self.acked or self.their_proposed
                                                     or self.their_acked or self.their_signed))

    def closing_tx(self, fee):
        mine, theirs = self.local_spec[0] // 1000, self.local_spec[1] // 1000
        if self.funder:
            mine -= fee
        else:
            theirs -= fee
        dust = max(self.cfg['dust'], self.peer.cfg['dust'])
        outputs = sorted((s, spk) for s, spk in ((mine, self.cfg['spk']), (theirs, self.peer.cfg['spk'])) if s >= dust)
        return 2, [(FUNDING_TXID + B.le(0, 4), b'', 0xffffffff)], outputs, 0

    def closing_hash(self, fee):
        v, ins, outs, lt = self.closing_tx(fee)
        return B.bip143(v, ins, outs, lt, 0, B.funding_script(self.bp['funding'], self.peer.bp['funding']), FUNDING_SAT, 1)

    def offer(self, fee):
        self.close_sent = fee
        self.send(u(39, 2) + FUNDING_TXID + u(fee, 8) + compact(self.secret['funding'], self.closing_hash(fee)))

    def maybe_close(self):
        if self.funder and self.close_sent is None and self.quiet():
            self.offer(self.cfg['close_fee'])

    def finish(self, fee, theirs):
        v, ins, outs, lt = self.closing_tx(fee)
        script = B.funding_script(self.bp['funding'], self.peer.bp['funding'])
        mine = compact(self.secret['funding'], self.closing_hash(fee))
        sigs = [der(mine) + b'\x01', der(theirs) + b'\x01']
        if self.peer.bp['funding'] < self.bp['funding']:
            sigs.reverse()
        self.closing = B.serialize(v, ins, outs, lt, [[b''] + sigs + [script]])
        self.state = 'closed'

    # ---- receiving
    def receive(self, m):
        t = int.from_bytes(m[:2], 'big')
        p = self.peer
        if t == 32:
            self.send(u(33, 2) + p.temporary + self.params(True) + bytes.fromhex('01021000'))
            self.state = 'accept sent'
        elif t == 34:
            self.local_sig = m[68:132]
            self.send(u(35, 2) + FUNDING_TXID + self.sigs(0, self.remote_spec)[0])
            self.state = 'funding'
        elif t == 35:
            self.local_sig = m[34:98]
            self.state = 'funding'
        elif t == 36:
            if self.state == 'funding':
                self.state = 'normal'
        elif t in (128, 130, 131):
            c = p.last_update(m)
            if t == 128:
                self.their_next_id += 1
            self.their_proposed.append(c)
        elif t == 132:
            self.local_spec = reduce(self.local_spec, self.acked, self.their_proposed)
            self.local_number += 1
            self.local_sig = m[34:98]
            self.their_acked += self.their_proposed
            self.their_proposed, self.acked = [], []
            self.send(self.revoke_message())
            self.maybe_close()
        elif t == 133:
            self.received_secrets.append(m[34:66])
            self.acked += self.signed
            self.signed, self.their_signed = [], []
            self.remote_spec = self.remote_next
            self.waiting = False
            self.maybe_close()
        elif t == 38:
            self.shutdown_received = True
            if not self.shutdown_sent:
                self.shutdown()
            self.state = 'shutdown'
            self.maybe_close()
        elif t == 39:
            fee, sig = int.from_bytes(m[34:42], 'big'), m[42:106]
            self.close_received = fee
            agree = fee == self.close_sent or self.cfg['close_min'] <= fee <= self.cfg['close_max']
            if not agree:
                counter = self.cfg['close_fee'] if self.close_sent is None else (fee + self.close_sent) // 2
                if counter not in (fee, self.close_sent):
                    self.offer(counter)
                    return
            if fee != self.close_sent:
                self.offer(fee)
            self.finish(fee, sig)
        elif t == 136:
            next_commit = int.from_bytes(m[34:42], 'big')
            if self.waiting and next_commit == self.remote_number:
                for c in self.signed:                 # the updates the lost commitment_signed covered
                    self.send(self.update(c))
                self.send(self.commit_message())

    def last_update(self, m):
        """The change an update message carries, as the peer recorded it."""
        t, hid = int.from_bytes(m[:2], 'big'), int.from_bytes(m[34:42], 'big')
        if t == 128:
            return ('add', hid, int.from_bytes(m[42:50], 'big'), m[50:82], int.from_bytes(m[82:86], 'big'))
        return ('fulfill', hid, m[42:74]) if t == 130 else ('fail', hid)

    def disconnect(self):
        """BOLT 2: forget every update no commitment_signed covered."""
        self.next_id -= sum(1 for c in self.proposed if c[0] == 'add')
        self.their_next_id -= sum(1 for c in self.their_proposed if c[0] == 'add')
        self.proposed, self.their_proposed, self.out = [], [], []

    def held(self):
        """HTLCs this node still tracks: in either commitment or in flight."""
        ids = {(h[0], h[1]) for h in self.local_spec[2] + self.remote_spec[2]}
        ids |= {('out', c[1]) for c in self.proposed + self.signed + self.acked if c[0] == 'add'}
        ids |= {('in', c[1]) for c in self.their_proposed + self.their_acked + self.their_signed if c[0] == 'add'}
        return len(ids)


def config(alice):
    return {'dust': 546, 'reserve': 10000, 'htlc_min': 1000, 'delay': 144 if alice else 720, 'max_accepted': 4,
            'close_fee': 700 if alice else 500, 'close_min': 600 if alice else 300, 'close_max': 900 if alice else 600,
            'spk': b'\x00\x14' + (b'\xaa' if alice else b'\xbb') * 20}


def transcript():
    bolts.text('02-peer-protocol.md')                  # checks the pinned BOLT 2's hash
    a, b = Node('Alice', True, b'\x11' * 32, config(True)), Node('Bob', False, b'\x22' * 32, config(False))
    a.peer, b.peer = b, a
    opener = (FUNDING_SAT * 1000 - PUSH_MSAT, PUSH_MSAT, [])
    a.local_spec = a.remote_spec = (opener[0], opener[1], [])
    b.local_spec = b.remote_spec = (opener[1], opener[0], [])
    a.remote_spec, b.remote_spec = (opener[1], opener[0], []), (opener[0], opener[1], [])
    lines = []

    def step(x, y):
        m = x.out.pop(0)
        lines.append(f'  {x.name} -> {y.name}: {NAMES[int.from_bytes(m[:2], "big")]}, {len(m)} bytes, '
                     f'sha256 {sha(m)[:4].hex()}')
        y.receive(m)

    def pump():
        while a.out or b.out:
            while a.out:
                step(a, b)
            while b.out:
                step(b, a)

    def commitment(x):
        outs, fee = x.outs(True, x.local_number, x.local_spec)
        txid = x.view(True, x.local_number).txid(outs)[::-1].hex()
        parts = ''.join(f' {o[3]} {o[0]}' + (f' (expiry {o[5]})' if o[3] in ('offered', 'received') else '') for o in outs)
        lines.extend([f"  {x.name}'s commitment {x.local_number}: {txid}", f'   {parts}, fee {fee}'])

    def both():
        commitment(a)
        commitment(b)

    p0, h0 = preimage(1)
    p1, h1 = preimage(2)
    p2, h2 = preimage(3)
    lines.append('Open: Alice funds 1,000,000 satoshis and pushes 200,000 to Bob')
    a.open()
    pump()
    lines.append(f"  Alice's wallet funds {FUNDING_TXID[::-1].hex()}:0")
    a.fund()
    pump()
    lines += [f'  channel_id {FUNDING_TXID.hex()}', '  the funding transaction is 3 blocks deep']
    a.ready()
    b.ready()
    pump()
    lines.append(f'  state: Alice {a.state}, Bob {b.state}')
    both()

    lines.append('Alice offers two HTLCs: 50,000 and 20,000 satoshis')
    a.add(50000000, h0, 500)
    a.add(20000000, h1, 510)
    a.commit()
    pump()
    b.commit()
    pump()
    both()

    lines.append('Bob fulfills the first and fails the second')
    b.remove('fulfill', 0, p0)
    b.remove('fail', 1)
    b.commit()
    pump()
    a.commit()
    pump()
    both()

    lines.append("Bob offers 30,000 satoshis; his commitment_signed is lost")
    b.add(30000000, h2, 600)
    b.commit()
    step(b, a)
    lost = b.out.pop(0)
    lines.append(f'  lost: {NAMES[int.from_bytes(lost[:2], "big")]}')
    a.disconnect()
    b.disconnect()
    lines.append(f'  reconnected: Alice forgot the add; HTLCs held: Alice {a.held()}, Bob {b.held()}')
    a.reestablish()
    b.reestablish()
    pump()
    a.commit()
    pump()
    lines.append('Alice fulfills it')
    a.remove('fulfill', 0, p2)
    a.commit()
    pump()
    b.commit()
    pump()
    both()

    # BOLT 2's rules, as this model reads them, for each attempt.
    lines.append('Messages and commands out of order')
    alice_msat = a.local_spec[0]
    fee_one = B.commit_fee(FEERATE, 1, False)
    rules = [
        # BOLT 2: an update_add_htlc's id must be the next one, here 2.
        ('update_add_htlc with id 9, not 2', 'BAD_ID' if 9 != b.their_next_id else 'OK'),
        # No second commitment_signed before the revoke_and_ack for the first.
        ('Alice signs twice', 'WAITING'),
        ('Alice signs with nothing to sign', 'NO_CHANGES' if not (a.proposed or a.their_acked) else 'OK'),
        # An HTLC is removed only once it is irrevocably committed to both
        # commitments; the add Bob just received is in neither.
        ('Bob fulfills an HTLC both have not yet signed', 'BAD_ID'),
        # The funder's balance less the amount and the fee of a commitment
        # with one more HTLC must keep the reserve Bob asked for.
        ('Alice offers 775,000 satoshis, into her reserve',
         'BAD_AMOUNT' if alice_msat - 775000000 < (b.cfg['reserve'] + fee_one) * 1000 else 'OK'),
        ('revoke_and_ack with no commitment outstanding', 'WRONG_STATE' if not a.waiting else 'OK'),
        ('closing_signed before shutdown', 'WRONG_STATE' if not a.shutdown_sent else 'OK'),
        ('open_channel on an open channel', 'WRONG_STATE' if b.state != 'init' else 'OK'),
    ]
    lines += [f'  {what}: {code}' for what, code in rules]

    lines.append('Bob could close alone with his latest commitment')
    ch = b.view(True, b.local_number)
    outs, _ = ch.outputs(b.local_spec[0], b.local_spec[1], FEERATE, [])
    digest = ch.commitment_sighash(outs)
    tx = ch.signed(outs, B.der_sign(b.secret['funding'], digest), der(b.local_sig))
    fund = B.p2wsh(B.funding_script(a.bp['funding'], b.bp['funding']))
    lines.append(f'  it spends the funding output: {runs(tx, fund, FUNDING_SAT)}, {len(tx)} bytes')

    lines.append('Close: Alice wants a 700-satoshi fee, Bob 300 to 600')
    a.shutdown()
    a.state = 'shutdown'
    pump()
    assert a.closing == b.closing
    lines.append(f'  closing fee {a.close_sent}, state: Alice {a.state}, Bob {b.state}')
    lines.append(f'  closing transaction {a.closing.hex()}')
    lines.append(f'  it spends the funding output: {runs(a.closing, fund, FUNDING_SAT)}')
    return lines


def main():
    lines = transcript()
    want = FIXTURE.read_text().splitlines()
    assert lines == want, '\n'.join(f'{x!r} != {y!r}' for x, y in zip(lines, want) if x != y) or 'line counts differ'
    print(f'{len(lines)} lines agree with {FIXTURE.name}')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', EXAMPLE, *mode, '--run-steps', RUN_STEPS], cwd=REPO,
                                 capture_output=True, text=True, check=True)
            assert got.stdout.splitlines() == lines, f'coax run {" ".join(mode)} differs'
            print(f'coax run {" ".join(mode) or "(debug)"}: {len(lines)} lines agree')


if __name__ == '__main__':
    main()
