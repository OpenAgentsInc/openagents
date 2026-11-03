#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: loads Bitcoin Core's own Python P2P serializer outside Coax)
"""Loads Bitcoin Core's functional-test P2P serializer,
test/functional/test_framework/messages.py, pinned in
coax/tests/vectors/upstream/bitcoin-core/messages.py, for Bitcoin chapter 65.

messages.py imports two helpers from the rest of Core's test framework:
siphash256, from test_framework/crypto/siphash.py (pinned beside it for
compact blocks, Bitcoin chapter 73, and loaded from there), and
assert_equal/assert_not_equal, given here as small stand-ins. The pinned
file runs unchanged:

  import core_messages
  M = core_messages.load()
  M.msg_version().deserialize(...)
"""
import hashlib
import sys
import types
from pathlib import Path

HERE = Path(__file__).resolve().parent
PINNED = HERE.parent / 'vectors/upstream/bitcoin-core/messages.py'
PIN = '44e3b8c702916c8ab7820db65cf38af801dd5b5a457e91cabdff95eba5d412a5'
SIPHASH = HERE.parent / 'vectors/upstream/bitcoin-core/siphash.py'
SIPHASH_PIN = '395fb94ebc7fb7d38bfe86bc3dbfb4df2e94d3c1e3cd1c41b66cc19d0d5e76e3'


def _assert_equal(a, b, *rest):
    if a != b or any(a != x for x in rest):
        raise AssertionError(f'not equal: {a!r} {b!r} {rest!r}')


def _assert_not_equal(a, b):
    if a == b:
        raise AssertionError(f'equal: {a!r}')


def _load_siphash():
    data = SIPHASH.read_bytes()
    assert hashlib.sha256(data).hexdigest() == SIPHASH_PIN, 'siphash.py differs from its pin'
    module = types.ModuleType('test_framework.crypto.siphash')
    module.__file__ = str(SIPHASH)
    exec(compile(data, str(SIPHASH), 'exec'), module.__dict__)
    return module


def load():
    data = PINNED.read_bytes()
    assert hashlib.sha256(data).hexdigest() == PIN, 'messages.py differs from its pin'
    if 'core_messages_pinned' in sys.modules:
        return sys.modules['core_messages_pinned']
    framework = types.ModuleType('test_framework')
    crypto = types.ModuleType('test_framework.crypto')
    siphash = _load_siphash()
    util = types.ModuleType('test_framework.util')
    util.assert_equal, util.assert_not_equal = _assert_equal, _assert_not_equal
    for name, module in [('test_framework', framework), ('test_framework.crypto', crypto),
                         ('test_framework.crypto.siphash', siphash), ('test_framework.util', util)]:
        sys.modules.setdefault(name, module)
    module = types.ModuleType('core_messages_pinned')
    module.__file__ = str(PINNED)
    exec(compile(data, str(PINNED), 'exec'), module.__dict__)
    sys.modules['core_messages_pinned'] = module
    return module
