# Private SPACE service

This directory owns the separate maintained deployment for #11814. It does not
reuse a disposable spike host. The native service, recovery, and capacity checks
must pass before this is ready for customer computers.

The key broker uses a dedicated Cloud Run identity and KMS key. Its ingress is
internal, its workload bearer and policy come from numeric Secret Manager
versions, and at most one instance serves eight requests at once. Only the key
broker identity can use its KMS key. Workers receive no broker identity.

Run `python3 deploy/space-service/broker.py foundation` to create the owned private
network, two subnets, NAT, IAP-only operator access, identity, and rotating KMS
key. The helper refuses unrelated resources with matching names. No public SSH
or management listener is opened.

Run `broker.py configure TENANTS.json` with a JSON list of allowed local SPACE
organization UUIDs. Retrying preserves the existing bearer and identical policy.
A different policy requires a reviewed new secret version: removing an
organization or key route can make its saved computers inaccessible. Retain old
KMS versions and routes while private data references them.

Build `key-broker-cloudbuild.yaml` with the full `_COMMIT`, then run
`broker.py deploy COMMIT sha256:DIGEST --policy-version N --token-version N`.
Deployment verifies source ancestry and the registry digest. Pin all numeric
secret versions; do not use `latest`.

`broker.py probe-host` creates an isolated, private, credential-free test VM that
deletes itself after two hours. Run the Rust `space-key-broker` example `probe`
there with the endpoint, mounted bearer file, namespace, and two distinct allowed
organization IDs. It never prints plaintext keys or credentials. Delete the
probe VM as soon as the check ends; its boot disk deletes with it.
