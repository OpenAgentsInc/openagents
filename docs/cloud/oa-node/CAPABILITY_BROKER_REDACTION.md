# oa-node Capability Broker Redaction

Status: Cloud MVP scaffold for `CND-022`

`oa-node broker redact` provides the local redaction boundary for broker logs,
receipts, and artifacts. It is intentionally strict: secret-looking inputs are
rejected unless the fixture explicitly carries `OPENAGENTS_FAKE_SECRET_OK`.

```bash
oa-node broker redact \
  --kind headers \
  --input ./headers.txt \
  --json
```

Supported kinds are:

- `headers`
- `url`
- `env`
- `config`
- `log`
- `receipt`

The broker writes only redacted artifacts and digest-only receipts:

```text
broker-redacted-artifacts/<kind>-<digest>.txt
broker-redaction-receipts.jsonl
```

Receipt fields include kind, input digest, redacted artifact path, redacted
digest, receipt digest, and emission time. Raw headers, URLs, env values,
config content, log lines, and receipt payload bodies are not written to the
receipt log.

Fixtures containing secret-looking data fail unless marked fake. Marked fake
fixtures are redacted line-by-line, and the resulting artifact is rejected if it
still contains raw secret, bearer token, API key, password, wallet seed, private
key, or `sk-` style token markers.
