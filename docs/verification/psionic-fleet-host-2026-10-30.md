# Fleet host byte effects — 2026-10-30

`psionic/platform/fleet-host.c` supplies C platform effects to chapter 25's
Coax runtime. It does not parse HTTP, choose Pylons, validate decisions or
construct protocol responses. The Coax entry is `coax_fleet_main(void)`.

The worker has a 512 MiB stack. The main thread receives SIGINT/SIGTERM and
publishes a stop flag; socket waits observe that flag at least every 100 ms.
Coax reports the process exit status through `fh_status`.

`fh_listen` can bind only IPv4 `127.0.0.1`. Its integer handles identify
one of 32 owned sockets, not arbitrary OS descriptors. Accepted sockets are
nonblocking and close-on-exec. One read exposes at most 65,536 bytes until
the next read or owning close; -1 is EOF, -2 is timeout, -3 is invalid/error.
Each stream has a separate output allocation, capped at 1 MiB. A partial
flush preserves the unsent offset for retry; close discards that stream's
staging. The single Coax worker owns handles and their lifetimes.

## Focused checks

`psionic/tests/fleet/host.sh` passed on this Mac (Clang) and on
`coderos-4080` (GCC/NixOS), with ASan, UBSan and `-Wall -Wextra -Werror`.
The same source files were used on both machines. Tests use actual loopback
TCP connections and independently receive/compare the emitted bytes:

- listener address, descriptor flags and invalid handles/arguments;
- HTTP-shaped request/response bytes, read timeout, EOF, and invalidated views;
- a 70,000-byte input split across bounded reads;
- separate pending replies for two connections and discarded staging on close;
- 1 MiB output bound, forced partial flush, and an exact no-duplication retry;
- argument bytes, wall clock, worker stack/lifecycle, stop and exit status;
- SIGTERM during a blocked accept. This initially failed at the deadline;
  bounded polling fixed it and the same test then passed on both platforms.

This qualifies host effects, not the chapter 25 HTTP/router end-to-end path.
The integrating Coax implementation and its fleet/Pylon interoperability have
separate checks and records. No public listeners, real identity or paid work
are involved in these host tests.
