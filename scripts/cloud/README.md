# Cloud run artifacts

To publish the Boat runner's logs, submitted patch, and exit evidence, set
`OA_ARTIFACT_BUCKET=gs://YOUR_PRIVATE_BUCKET` and `OA_ARTIFACT_ISSUE` to the
issue number before running `scripts/boat-run.sh`. Publication is opt-in:
review your command and patch for secrets before enabling it. Signed URLs
are bearer credentials visible to everyone who can read the issue. They
expire after 24 hours; the underlying objects remain private.

The local `gcloud` identity needs object-create and signing permissions.
Configure bucket lifecycle retention separately. Each run uses a unique
prefix. Both successful and failed command results are retained in
`~/.openagents/boat/run-*`. A publication failure prints that local path
without masking the original command exit code. Uploads are not atomic;
a partial upload can leave private objects without an issue comment.

For a GCE run, write the four selected files (`stdout.log`, `stderr.log`,
`change.patch`, and `evidence.json`) to a uniquely named directory, then run:

```sh
python3 scripts/cloud/publish-artifacts.py /path/run-ID \
  --bucket gs://YOUR_PRIVATE_BUCKET --issue ISSUE_NUMBER
```

The publisher never uploads a whole task store or credential directory.
Run its isolated tests with `python3 scripts/cloud/test_publish_artifacts.py`.
