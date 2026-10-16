# Signet TDX service

This research binary runs one tree and Bitcoin Core 30.2 inside the measured
Confidential Space image. Core validates signet blocks locally, has no wallet,
and listens only on guest loopback. Mutations wait for synchronization and a
confirmed current output. The service never accepts a host-supplied height.

`service.json` is compiled into the image and names the first signet run.
For a new run, configure fresh public keys and a tree ID from its creation
funding input, then rebuild and log the new image. Launch metadata
cannot replace the command, configuration, mounts, or environment.

The Bitcoin co-signing key and TLS key are generated separately inside the
guest. Public PKI evidence binds the tree-parameter hash, a caller challenge,
and SHA-256 of the actual TLS certificate. The client verifies Google-backed
TDX evidence before sending an operator signature and pins that certificate
for every subsequent request. Release evidence must pass its own check first.

Each transition uses the existing predicate and `Enclave::prepare`/`finish`.
Parties check the pre-signed unroll before signing. The signer obtains valid
receipts from two of three configured mirrors before releasing its signature.
Direct member exit requests and guardian HALT messages carry their own
signatures. There is no key import, signing-hash, debug, or height-setting API.

This first service deliberately starts a new key after every restart. It does
not restore the sealed key qualified in #11790: doing that safely also requires
restoring current chain state, pending exits, mirror state, and HALT. If this
service dies, clients use the old tree's pre-signed unroll after its CSV delay.
A restart creates a new tree and restarts any uninterrupted-service measurement.
The mirror deployment uses three private VMs in separate zones, each with
its own receipt key. They share one project and operator; they are not
independent operators.

```sh
cargo test --locked --manifest-path crates/layertree-tee-lab/server/Cargo.toml
LAYERTREE_EXAMPLE=signet_service scripts/layertree/build-image.sh COMMIT OUTPUT
LAYERTREE_SERVICE_IMAGE=REGISTRY@sha256:DIGEST scripts/layertree/signet-service.sh create
scripts/layertree/signet-service.sh status
scripts/layertree/signet-service.sh delete
```

The VM script permits only the research builder to reach port 8443, deletes
the VM after nine days, and exposes no SSH or Core RPC. The mirror script
restricts data ports to the service and builder. Confirm member payouts before
early deletion. Core exiting stops the service.

The separate `layertree-signet-mirror` binary stores public records and unrolls
with exclusive publication and fsync before signing a receipt. It reads its
own mode-0600 research key, never the tree co-signing key. Its GET paths match
`follow::HttpMirror` and serve the original public bytes.

Implementation checks have passed on the builder. Live TDX channel, confirmed
signet transitions, and failure recovery still need their own recorded results;
compilation and unit tests do not qualify those flows.
