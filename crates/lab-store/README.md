# Lab storage

`LabStore` binds operations to one authorized personal or project space.
Authentication and membership checks belong to the API. `CheckedDraft` runs a
kind's checks before a version can be saved. Concurrent edits on the same base
have one winner.

`FsBackend` requires an absolute private root outside Git. `CloudBackend` uses
SQLx PostgreSQL indexes and immutable Cloud Storage objects. The host supplies
its pool, bucket, and existing token provider; optional IAM signing provides
short-lived download URLs. SQLx's core and PostgreSQL crates avoid its optional
SQLite driver, which conflicts with the workspace's embedded wallet version.

Objects are written and verified before a conditional index update publishes
them. Interrupted writes can leave unreferenced objects. Indexes are limited to
32 MiB per space; files to 128 MiB; pages to 100 entries. Kind manifests set
additional document, version, argument, and hourly job limits.

A worker claims a job, then records `start` before any effect. An unused expired
lease returns to the queue. An expired started job becomes `uncertain` and
cannot run again automatically. Lease attempts reject stale workers. Completed
results can be retried with the same lease and result.

Run `cargo test -p lab-store`. Set `LAB_STORE_TEST_DATABASE` to a disposable
loopback PostgreSQL database to include cloud conformance with a fake GCS HTTP
server. The same suite checks filesystem persistence, concurrent versions,
job retries, lease loss, ordered events, file ranges, and space isolation.

`lab-store-copy --source /absolute/export --owner ACCOUNT` reads an exported
CAD chat-bucket layout and prints counts and file checksums. Review its checksum,
then add `--target /absolute/copy --apply CHECKSUM` to make a verified copy.
The source is never changed. The copied manifest preserves source paths and
points to immutable objects; selecting a live destination requires the API's
CAD integration. Use `--project PROJECT` for a project export.
