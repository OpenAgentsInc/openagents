# Chapter21 public pool input effects

`psionic/platform/mesh-public-input.c/.h` is a separate public byte transport.
It does not call or change private `mw_*` witness input. Coax owns protobuf
version, policy, original event signatures and totals validation.

The `mpub_stdin_read(timeout_ms)` / `mpub_stdin_byte(index)` /
`mpub_stdin_clear()` API reads one EOF-framed binary bundle into owned static
storage at most 2MiB. Regular files, FIFOs and AF_UNIX sockets are allowed;
TTYs, devices, AF_INET sockets and invalid descriptors are refused before
reading. Return values distinguish timeout (-2), I/O/unsafe input (-3), and
overlimit (-4). Partial failure wipes the buffer. The caller retains fd0;
original file flags are restored. Positive timeouts share one monotonic
budget across all chunks; zero is a bounded nonblocking drain.

The actual focused C gate passed on Apple clang 21.0.0 and Linux GCC 15.2.0,
with `-Wall -Wextra -Werror`, ASan and UBSan; no warnings were suppressed.
Cases cover empty/binary/chunked input, exact 2MiB and 2MiB+1, regular-file
and pipe capacities, AF_UNIX carriage, unchanged flags, wiping and invalid
getters, partial timeouts, a continuing drip writer that cannot restart the
total deadline, and TTY/device/TCP/closed-fd refusal. The pseudo-terminal
is test-owned and never connected to the owner's screen.

```sh
psionic/tests/mesh21/check_public_input.sh /tmp/oct30-mesh-public-input-mac
# Linux, fresh first-party temporary copy:
COAX_CC=/run/current-system/sw/bin/gcc psionic/tests/mesh21/check_public_input.sh /tmp/coax-mesh-public-input.AH9Pp6/build
```

This is a host-effects qualification only. No public bundle or private
witness is logged, and it does not claim the pool's Coax admission path
has completed its separate signed end-to-end gate.

Exact input SHA-256:

- `psionic/platform/mesh-public-input.c`: `40740d56b10db9e6560b4361d53ad9c43dbbbd68ccbfcc3dd4e5d431c1244599`
- `psionic/platform/mesh-public-input.h`: `7f433bbb98114c905e5731b5f6106868ad37f5c934ed83e390967dc30c435766`
- `psionic/tests/mesh21/mesh_public_input.c`: `159996a23389499373344fd400a47292533b4d51435dbef3c3f62d7ca371fa62`
- `psionic/tests/mesh21/check_public_input.sh`: `ac5ae885ef3ca0672f0588a542a43ad2eae7843b6f62e05739d23a50d7b4576f`
