# Chapter 20 public HTTP rehearsal

`result.json` records four cases against the frozen sanitized binary. The
remote case used `wss://relay.openagents.com` and the real Mac Pylon. The
fallback case intentionally requested immediately after fresh HTTP startup,
before relay discovery, with discovery wait set to zero; it therefore proves
explicit same-identity local fallback, not a simulated remote answer. The two
negative cases return a truthful refusal instead of substituting identities.

`live-result.json` is an additional real Linux Pylon answer through the
retained loopback adapter. `live-run.json` records its process/command/stop
metadata at test time; `live-listener.txt` confirms only127.0.0.1:8798 listens.
The buyer key exists only in each native process's memory. No real identity,
private label or money is present in these records.

`rehearsal.py.txt` is the exact operator harness snapshot. It independently
checks HTTP status, answer shape/probabilities, exact model identity, and the
known provider/slug. Coax performs signed event verification, correlation,
NIP-44 decryption and remote answer admission in the native executable.

Try the retained adapter from this Mac while its Pylons are online:

```sh
curl -sS http://127.0.0.1:8798/v1/systemone \
  -H 'Content-Type: application/json' \
  -d '{"model":"psionic-toy","state":"Water is wet.","questions":{"wet":{"type":"choice","criteria":{"yes":null,"no":null}}}}'
```

The returned `service` identifies the actual route: `pylon:SLUG` and provider
for remote work, or `psionic` for explicit local fallback. Neither a toy
model's choice nor this demo is a model-quality claim. The production cloud
API is not connected to this loopback process.
