---
id: docker.cp-directory-nesting
version: 1
kind: tool
title: docker cp nests a directory that already exists
summary: >-
  docker cp SRC DEST copies a directory into DEST/SRC-name when DEST already
  exists, but to DEST itself when it doesn't, so repeating a copy nests it.
  Copy a directory's contents with SRC/. to get the same result either way.
tags: [docker, docker-cp, containers, copy, directories, paths]
applies_when: >-
  Commands copy files or directories into or out of a container with docker
  cp, or a copied tree ends up one level deeper than expected.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Docker documentation, docker container cp"
evidence: []
---

## Details

For `docker cp SRC CONTAINER:DEST` with `SRC` a directory:

| `DEST` | Result |
| --- | --- |
| doesn't exist | `DEST` is created with `SRC`'s contents |
| exists, a directory | `SRC` is copied into it, as `DEST/<name of SRC>` |
| exists, a file | error: can't copy a directory to a file |

`SRC/.` (a trailing `/.`) copies the directory's contents into `DEST`,
whether or not `DEST` exists. That's usually what you want when refreshing a
tree.

For a file: if `DEST` ends in `/`, it must be an existing directory; if
`DEST` is an existing directory, the file goes inside it; otherwise the file
is written as `DEST`.

The same rules apply from a container to the host. `cp -r` on Linux behaves
the same way for an existing destination directory.

## How to check

After a copy, list the destination (`docker exec CONTAINER ls -la DEST`)
and confirm the files are at the level you expect, not under an extra
directory named after the source.
