# Memory-only private witness input

`psionic/platform/mesh-input.c` implements the chapter 21 checker's inherited
input effects. It has no main and no Nostr/protobuf/decision logic. There is
no witness file or path argument, file output, diagnostic output or key API.
The Coax caller observes the input kind, admits its private input profile,
and decodes/checks the received bytes.

`mw_stdin_kind` distinguishes FIFO, AF_UNIX socket, regular file, TTY and
other/error. An AF_INET socket is never called a private socket. Before
reading, the effect duplicates and rechecks the actual descriptor, then
accepts only FIFO/AF_UNIX input capabilities. It never reads regular-file,
TTY, device or network-socket contents. The caller owns an exclusively read,
private inherited channel, such as the process host's anonymous stdin pipe.

`mw_stdin_read` receives one complete binary witness until EOF, at most
262144 bytes, with a total monotonic timeout of 0..60000 ms. Embedded zero
bytes are preserved. Partial timeout/overflow/error clears the buffered
view; `mw_stdin_clear` uses volatile stores over the fixed buffer. Descriptor
flags are restored and duplicate descriptors closed on completion. This
qualifies that local buffer's behavior, not whole-process secret erasure or
production signer handling. There is no production-key approval here.

## Actual checks

The exact same final source passed on both machines:

- Mac: Apple clang 21.0.0, `psionic/tests/mesh21/check_input.sh`.
- coderos-4080: GCC 15.2.0, identical sources copied to a fresh `/tmp` directory
  and compiled/run with the script's same flags.

Both use `-std=c11 -O1 -g -Wall -Wextra -Werror`, ASan/UBSan,
`-fno-sanitize-recover=all` and `-fno-omit-frame-pointer`.
No sanitizer findings or warning exemptions. Tests exercise binary/chunked
input, empty EOF, exact 262144-byte capacity, one-byte overflow, total and
partial timeout, unsafe descriptor rejection before bytes, AF_UNIX socket
input, input-view clearing and descriptor-flag restoration. TTY tests use an
unseen newly created pseudo-terminal, not the owner's terminal.

Source identities, SHA256:

- `mesh-input.c`:
  `0d131c65a97da8d2eeca2fb1115d0abf7a51af794658c38a7e332e37450e426e`.
- `mesh-input.h`:
  `39d8779e402e82636f80e33da4286cb2936433aa23b0285b397f505dd2ed6470`.
- `mesh_input.c` harness:
  `2f82d167e565882357ecb9242994ad3b6b8fc23131365ee55e1abd74d1507728`.

The Linux warning gate initially exposed compressed statement formatting;
that was corrected in source, then both final-source platform gates passed.
No passing gate was repeated for an unrelated main commit. The live
chapter 20 adapter and Pylons were untouched.
