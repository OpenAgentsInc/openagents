# SPACE key broker

This Rust service implements the public SPACE v2 intermediate-key protocol at
`de87b436c35dac056564bdd5b2a113c9046e189b`. It generates fresh random 256-bit
keys and wraps them with Google Cloud KMS. Authenticated context binds each
wrapped key to its tenant and opaque key ID. Historical routing aliases keep
old keys usable after migration; KMS versions must remain enabled while data
still references them.

Run behind the private Cloud Run TLS endpoint. Set `SPACE_KEY_BROKER_CONFIG`
to the policy JSON path and `SPACE_KEY_BROKER_TOKEN_FILE` to the mounted service
credential. Policy names the tenant namespace, allowed organization UUIDs,
active routing alias, and alias-to-KMS-key map. The runtime uses its dedicated
service identity through the metadata server, without an interactive login.

The workload credential authorizes the SPACE credential authority for the
listed local organizations. Any supplied federated authorization evidence is
rejected. Key release specifies zero plaintext caching. Backend outages and
permission failures return unavailable; they never declare key revocation.

The implementation follows Google's [KMS integrity checks](https://docs.cloud.google.com/kms/docs/data-integrity-guidelines)
and [authenticated context](https://docs.cloud.google.com/kms/docs/additional-authenticated-data).
The pinned public wire contract is in `proto/intermediate_key_broker.proto`
under the upstream Apache-2.0 license.
