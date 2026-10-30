# Chapter21 public pool output effects

`mesh-public-output.c/.h` adds only `mpub_stdout_tty()` (one or zero) and
`mpub_stderr_byte(byte)` (zero or -3). Coax controls structured output,
refuses binary bundles on a TTY, and supplies public IDs or fixed diagnostics;
this host never forwards opaque process/stdin data. The existing fleet host
ignores SIGPIPE before use, so broken stderr pipes report an I/O error.

Focused tests passed on Apple clang 21.0.0 and Linux GCC 15.2.0 with
`-Wall -Wextra -Werror`, ASan and UBSan. They cover an owned pseudo-terminal,
pipe and closed stdout; all 256 binary stderr bytes; negative/overflow byte
bounds; a full nonblocking pipe with unchanged flags; and broken/closed
stderr. No owner terminal, private witness or real key was used. The already
qualified public/private readers and their tests were unchanged and were
not rerun.

```sh
psionic/tests/mesh21/check_public_output.sh /tmp/oct30-mesh-public-output-mac
COAX_CC=/run/current-system/sw/bin/gcc psionic/tests/mesh21/check_public_output.sh /tmp/coax-mesh-public-input.AH9Pp6/output-build
```

Exact input SHA-256:

- `psionic/platform/mesh-public-output.c`: `842f1330afa37654b1d125983040407bd2453ea32a4906c7285af438808b8fda`
- `psionic/platform/mesh-public-output.h`: `ef696466a4372c92052be90d8cde5ba6920b40f01cccc665cd9701af2a427c6e`
- `psionic/tests/mesh21/mesh_public_output.c`: `f9053fbe03ad95f17a60ff8246422fdd734030fa1efb3de6f02772791cc53688`
- `psionic/tests/mesh21/check_public_output.sh`: `5b472922b1c22dae49ac4e9b2ea18e6dbdf68464cc190dbb85290b5e1e4cfc58`
