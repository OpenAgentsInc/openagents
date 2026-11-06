# Wayland committed text and clipboard ownership

This #12869 component extends `3eb6939a61`'s XKB input adapter. It uses
the actual production `wayland-text-sdk.h`, generated text-input-v3 client
protocol, real Wayland client objects, XKB compose state and physical pipe
descriptors. It opens no compositor, window, GPU or audio device. Ordinary
window integration and the final default switch remain separate gates.

Run once for normal, then with `--sanitize` for ASan/UBSan/LSan, inside the
declared Linux SDK environment:

```sh
COAX_JOBS=1 COAX_CHECK_JOBS=2 COAX=/path/to/admitted/coax \
nix-shell -p gcc pkg-config wayland wayland-scanner wayland-protocols libxkbcommon vulkan-headers \
  --run 'nice -n15 coax/web/tool.sh aethyr/host/check-wayland-text.coax "$PWD" /tmp/wayland-text-new'
```

Use a new output directory for each run. The runner bounds each emit/build
and listener process, records its argv/status, hashes the compiler, product
sources, protocol XML/products and binaries, and checks that every input
remains unchanged. `sources.json` is the source fence. Linux uses the
previously admitted 0.50 compiler at `8012c3c9…/coax`; these records do not
claim a new compiler-source admission. The accompanying Coax policy and
receipt authorities were checked with the admitted Mac 0.50 image
`c82201e2…/coax` (SHA-256 `b5872254e02de4b2fb84f039e81f602090f5526301c036c077004294a7e580eb`).

Both frozen runs pass every one of 31 rows / 124 fields:

- The text-input-v3 enable/commit serial is counted. Preedit produces no
  inbox text; a complete Japanese/emoji commit arrives once. A done for older
  compositor state in the same enabled field remains valid. Old-field done,
  stale engine identity, malformed/oversized UTF-8 and unsupported surrounding
  deletion cannot publish a prefix. Surrounding text is deliberately not
  advertised by this ABI.
- Actual `wl_data_offer_receive` requests transfer a physical pipe FD through
  Wayland's socket. Empty/held input publishes nothing before EOF; complete
  UTF-8 publishes once. Malformed text, embedded NUL, owner/field/focus
  changes and five-second expiry close the owned transfer. Exactly 2 MiB
  physically accumulates without publication, then the next byte refuses the
  whole transaction and releases the buffer.
- A cancelled outgoing source retains its old UTF-8 bytes while its send is
  pending. A replacement selection cannot change those bytes. Actual send
  completion releases the old source. A closed reader produces a refusal
  without killing the process with SIGPIPE. Four source cells bound retention;
  further copies refuse. All payload owners close to zero.
- The real window callbacks cancel actual XKB composition on pointer-down,
  Enter, engine identity change, explicit replacement invalidation and
  keyboard focus loss. Replacement invalidates the text epoch while preserving
  actual keyboard focus; it does not wait for another compositor enter event.

The fixed SDK cells are bounded metadata; retained source payloads are
separately limited to 8 MiB, with four send descriptors and one 2 MiB paste
buffer. Incoming IME transactions have a fixed 4,097-byte SDK buffer. No
intermediate preedit becomes a key press. Compositors lacking text-input-v3
retain local XKB compose. Clipboard capability/refusal reports through the
existing host 604 error callback.

`wayland-text.coax` kills all **107/107** root mutants; the independent
124-field receipt gate kills **138/138**, without equivalent exclusions.
The driver has **107/107** passing checks. Shell syntax, the updated Nix
expression parse and tools-check (1,809 files, zero missing reasons) pass.
The initial compile-time 2 MiB array exceeded the ordinary check's step
budget; that check was split into scalar allocation bounds and the physical
2 MiB SDK pipe observation. No step budget was raised. Its original failed
mutation gates and corrected results remain in the raw folder.

Raw observations, source fences, command records and diagnostics are under
[`wayland-text/`](wayland-text/). The two frozen SDK runs have empty sanitizer
diagnostics. These synthetic compositor callbacks exercise the real client
SDK boundary; they do not claim an interactive third-party IME engine session
or a Nix image build.
