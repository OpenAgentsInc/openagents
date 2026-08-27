# harbor-runner image

The `harbor-runner` image is the pinned container for OpenAgents Gym runs. It
holds:

- Python 3.12
- Harbor (installed from `docker/harbor-runner/requirements.txt` at build time)
- `bench/adapters/openagents_coder.py`
- `bench/post_gym_run.py`
- `bench/run-suite.sh`
- `bench/suites/`

## Digest pinning convention

The base image is pinned by SHA-256 digest in the `FROM` line of the
`Dockerfile`. To update the base image:

1. Pull the new tag and read its digest:

   ```sh
   docker pull python:3.12-slim-bookworm
   docker image inspect python:3.12-slim-bookworm --format='{{index .RepoDigests 0}}'
   ```

2. Replace the `BASE_IMAGE` default in `Dockerfile` and update the comment.

3. Run `docker/harbor-runner/build.sh` and record the new local image id.

`gym env pull` builds this image locally, records the image id in
`~/.openagents/gym/harbor-runner/digest.json`, and skips the build when the
context and the recorded id have not changed.
