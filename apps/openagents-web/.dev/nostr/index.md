# Nostr probe

Local-only read verification. The engine hotbar and keys 1–5 select the captured
event, changed ID, changed content, changed signature, and an unsupported kind.
VERIFIED means that this exact event's ID and public signature verified; it
does not grant freshness, author, project or action authority.

This hidden source folder is omitted by normal site deployment and is not
reachable through the ordinary site's dev-server dotfile routes. Serve it as
its own app from the repository root:

```sh
coax/web/serve.sh apps/openagents-web/.dev/nostr --listen 127.0.0.1:18915
```

The original object bytes come from the existing public relay capture. The
four negative cases are explicit tampered copies. No signer or private key is
loaded. Every result comes from `nostr_event_verify_vartime`; no other event
decoder or verifier is implemented here.

Build and run the explicit tests separately (Python is test tooling only):

```sh
apps/openagents-web/.dev/nostr/build.sh
python3 apps/openagents-web/.dev/nostr/test.py
coax/bin/coax test apps/openagents-web/.dev/nostr/probe.coax
coax/bin/coax test apps/openagents-web/.dev/nostr/verifier.coax
coax/web/check-visuals.sh apps/openagents-web/.dev/nostr
```

The renderer and verifier compile as separate Coax libraries because their
existing flat namespaces both contain names such as `field` and `frame`.
The C face transfers four scalar observations between the two libraries.
This preserves the already verified core's source bytes and uses no alternate
event decoder. The tests compile that scalar ABI against the generated header.
