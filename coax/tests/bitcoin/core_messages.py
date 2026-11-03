#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: loads Bitcoin Core's own Python P2P serializer outside Coax)
"""Loads Bitcoin Core's functional-test P2P serializer,
test/functional/test_framework/messages.py, pinned in
coax/tests/vectors/upstream/bitcoin-core/messages.py, for Bitcoin chapter 65.

messages.py imports two helpers from the rest of Core's test framework:
siphash256 (for compact blocks, which chapter 65 doesn't use) and
assert_equal/assert_not_equal. They are given here as small stand-ins, so
the pinned file runs unchanged:

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


def _assert_equal(a, b, *rest):
    if a != b or any(a != x for x in rest):
        raise AssertionError(f'not equal: {a!r} {b!r} {rest!r}')


def _assert_not_equal(a, b):
    if a == b:
        raise AssertionError(f'equal: {a!r}')


def _siphash256(k0, k1, h):
    raise NotImplementedError('compact blocks are not used here')


def load():
    data = PINNED.read_bytes()
    assert hashlib.sha256(data).hexdigest() == PIN, 'messages.py differs from its pin'
    if 'core_messages_pinned' in sys.modules:
        return sys.modules['core_messages_pinned']
    framework = types.ModuleType('test_framework')
    crypto = types.ModuleType('test_framework.crypto')
    siphash = types.ModuleType('test_framework.crypto.siphash')
    util = types.ModuleType('test_framework.util')
    siphash.siphash256 = _siphash256
    util.assert_equal, util.assert_not_equal = _assert_equal, _assert_not_equal
    for name, module in [('test_framework', framework), ('test_framework.crypto', crypto),
                         ('test_framework.crypto.siphash', siphash), ('test_framework.util', util)]:
        sys.modules.setdefault(name, module)
    module = types.ModuleType('core_messages_pinned')
    module.__file__ = str(PINNED)
    exec(compile(data, str(PINNED), 'exec'), module.__dict__)
    sys.modules['core_messages_pinned'] = module
    return module
