# Bounded production observation: 21:11–21:21 UTC

The qualified muted/headless capture sampled the actual production PYLONS
view at approximately five-minute intervals. Each receipt preserves two
read-only Coax-state observations, ten seconds apart, and original signed
public beacon frames. No app data was injected; there were no deployments,
native inference calls or repeated source qualification gates in this slice.

All three final observations had **six online providers / 19 advertised free
slots**, including the same four exact cloud provider keys and checkpoint
identities in the [initial verified roster](../README.md). Each cloud row was
live, status 0, with a positive remaining lease and unchanged generation.
No cloud row disappeared or expired, and there were no browser errors.

| Receipt / final UTC observation | Elephant lease | Mammoth lease | GPT-OSS lease | Clef lease |
| --- | --- | --- | --- | --- |
| `211137.json` — 21:11:37.299 | 139 s | 139 s | 134 s | 186 s |
| `211641.json` — 21:16:41.352 | 141 s | 141 s | 136 s | 188 s |
| `212140.json` — 21:21:40.400 | 146 s | 146 s | 141 s | 194 s |

The four cloud Pylons still map to **two physical cloud hosts**: three GPU
providers share the RTX PRO VM, and confidential Clef uses the separate TDX
CPU VM. The two other rows are Mac advertisements, not extra cloud machines.
The physical-host mapping comes from the operator deployment record, not from
untrusted display labels or a sum of advertised memory/slots.

This bounded sampler is stopped. These are point observations spanning ten
minutes; they do not assert uninterrupted health between samples or completion
of the future six-hour interval. Managed model probes and the owning thread's
heartbeat continue independently through 03:09:43 UTC.
