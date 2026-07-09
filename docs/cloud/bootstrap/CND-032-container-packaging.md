# CND-032 Container Packaging

> **Historical bootstrap note (#8591).** Kept for archaeology and ops memory.
> Active Cloud implementation is in the public monorepo (`crates/*`,
> `docs/cloud/`). Deprecated authority names: **Vortex** → Worker/Khala Sync;
> **Treasury product** → Worker credits + MDK/Nexus payout bridge only;
> **Nexus-as-registry** → Worker/Khala Sync (CLI may still say `nexus`).
> Do not treat this note as current product-authority ownership.

Status: implemented scaffold

This repo packages the managed Cloud binaries as reproducible local container
images:

- `oa-node`
- `oa-workroomd`

The images are private managed-cloud infrastructure images. They do not carry
contributor Pylon wallet UX, public provider persona state, or raw secrets.

## Files

```text
docker/oa-node.Dockerfile
docker/oa-workroomd.Dockerfile
scripts/build-cloud-images.sh
```

## Local Build

```bash
scripts/build-cloud-images.sh --tag local
```

The script builds:

```text
openagents-cloud/oa-node:local
openagents-cloud/oa-workroomd:local
```

To tag for Artifact Registry:

```bash
scripts/build-cloud-images.sh \
  --registry us-central1-docker.pkg.dev/$PROJECT_ID/oa-cloud \
  --tag "$(git rev-parse --short HEAD)" \
  --push
```

## Image Metadata

Both images set OCI metadata:

- `org.opencontainers.image.title`
- `org.opencontainers.image.description`
- `org.opencontainers.image.vendor`
- `org.opencontainers.image.source`
- `org.opencontainers.image.created`
- `org.opencontainers.image.revision`
- `org.opencontainers.image.version`

The build script fills those labels from UTC build time, Git revision, and the
selected tag.

## Secret Boundary

The build script deliberately accepts no secret arguments.

Runtime secrets must be supplied through one of these paths:

- scoped platform identity;
- local broker or gateway;
- mounted secret files outside the image;
- short-lived workroom grants resolved by the runner.

Do not bake provider tokens, wallet material, bearer tokens, Codex auth files,
or private topology config into these images.

## Runtime Defaults

`oa-node`:

```text
OPENAGENTS_CLOUD_NODE_HOME=/var/lib/openagents/oa-node
ENTRYPOINT ["/usr/local/bin/oa-node"]
CMD ["status", "--json"]
```

`oa-workroomd`:

```text
OPENAGENTS_CLOUD_WORKROOM_HOME=/var/lib/openagents/oa-workroomd
ENTRYPOINT ["/usr/local/bin/oa-workroomd"]
CMD ["status", "--json"]
```

Both images run as non-root users.
