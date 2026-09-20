# Reproducible Kev artifacts

[`fetch-kev-artifacts.sh`](../../scripts/fetch-kev-artifacts.sh) reads an
`artifact-lock.json` beside each historical fixture manifest. It fetches
public files from full Hub commit IDs, verifies source inputs before head
conversion, and verifies every cached file before doing network work.
The lock includes the base `config.json` and every `model*.safetensors`
shard that the Rust loader reads. Extra base shards are refused.

The historical `manifest.json` files, golden probabilities, and measurement
rows remain unchanged. The new locks add acquisition provenance; they do
not relabel a historical checkpoint as the current release.

## Recover a historical bundle

Use a fresh root when recovering an existing directory that fails
verification:

```sh
KEV_ARTIFACTS="$HOME/work/kev-artifacts-recovered" \
  ./scripts/fetch-kev-artifacts.sh kev-0.5b kev-0.6b kev-4b kev-8b
```

The default root remains `../kev-artifacts` beside the workspace. Supply
`--root <directory>` or `KEV_ARTIFACTS` to change it. A conversion environment
is created under that root only when converted head files are missing.
The verified recipe uses Python 3.12, PyTorch 2.8.0, and safetensors 0.8.0.
To use an existing environment:

```sh
KEV_CONVERTER_PYTHON=/path/to/environment/bin/python \
  ./scripts/fetch-kev-artifacts.sh --root /path/to/artifacts kev-4b
```

The converter uses `torch.load(weights_only=True)` only after the source
head passes its pinned digest and size checks. Both converted outputs must
match before either is published. Incomplete downloads remain unaccepted;
a retry downloads them from the beginning. Existing mismatched files are
refused, not overwritten. Move the named file aside or choose a fresh root.
All checkpoints are public; the downloader does not read or transmit an
API token.

| Variant | Adapter revision | Base revision |
| --- | --- | --- |
| `kev-0.5b` | `edf1dc6d7f8d983c0adfd251e80a686e5539fc61` | `060db6499f32faf8b98477b0a26969ef7d8b9987` |
| `kev-0.6b` | `30902c2bbc113145cc150199b2b999f64ef7f0b8` | `da87bfb608c14b7cf20ba1ce41287e8de496c0cd` |
| `kev-4b` | `1a0cb0a0c4ea77e259cd215a3fb85d29edcc499e` | `906bfd4b4dc7f14ee4320094d8b41684abff8539` |
| `kev-8b` | `6466fbd425be23b9634b19295afa3ea220f5d11a` | `49e3418fbbbca6ecbdf9608b4d22e5a407081db4` |

The 0.5B fixture manifest did not pin `head_meta.json`. Its new lock pins
the metadata converted from the published source head. A locally edited
metadata file, including one that adds the default `option_isolation`
field, fails the byte check even when its behavior is equivalent. Recover
into a fresh directory rather than rewriting historical evidence.

## Pin a replacement checkpoint

1. Select full immutable adapter, base, and reference-code commits. Inspect
   the head metadata to identify the base, and verify that the claimed base
   matches the actual checkpoint used to generate fixtures.
2. Create a separate lock using the historical lock schema. Record SHA-256
   and byte size for every source file, including `head.pt`; use Hub LFS
   SHA-256 metadata for large files and hash downloaded small files.
   Record all base shards and `config.json` independently.
3. Verify the raw source hashes before conversion. Record the two converted
   output hashes and the conversion recipe. Set `variant` and the adapter
   `directory` to a revision-specific candidate name, such as
   `kev-4b-c4bfa11b0dc0`. Keep historical directories and locks intact.
4. Fetch and verify the candidate with
   `./scripts/fetch-kev-artifacts.sh --lock /path/to/artifact-lock.json`.
   The command prints the canonical lock digest and writes a copy into the
   adapter directory only after the complete bundle passes verification.
5. Generate fixtures from the pinned reference checkout into a new fixture
   directory, following the [fixture instructions](../../crates/kev/fixtures/README.md#regenerating).
   Retain the lock beside the generated manifest. Run real-weight conformance
   before workload scoring or changing a serving default.

The lock digest identifies the acquisition record, including its paths and
recipe. The server separately hashes the bytes it loads and publishes that
content identity through model discovery. Gym retains it with the numerical
execution settings. See [model identity](../gym/model-identity.md) for the
different guarantees of acquisition verification and runtime reporting.

## Verification

`python3 scripts/test_fetch_kev_artifacts.py` runs nine small regression
tests without network access or PyTorch. It covers moving `main`, cold and
warm agreement, corrupted cached inputs, incomplete downloads, invalid
conversion output, a mismatched base selection, unexpected base shards,
and retry behavior. The manual
gate runs these tests before Rust checks.

On 2026-09-20, all four adapters and tokenizers matched the historical
fixture digests at the revisions above. Raw heads were downloaded and
verified against Hub SHA-256 metadata, and conversion reproduced every
historically pinned head and metadata digest. All installed base files
matched the pinned Hub content. The new 0.5B metadata digest supplements
the historical manifest's missing field.

A complete 0.5B bundle was recovered into an empty root, including its base
weights. A second run made no downloads and printed the same lock digest.
The three Qwen3 variants also passed verification in their existing
directories, including all base shards.

## Reviewed replacement 4B

The separately named `kev-4b-c4bfa11` lock and fixtures pin the replacement
4B at `c4bfa11b0dc07691884f2d97f1c4c4c05c92e416`. Fetch it explicitly:

```sh
./scripts/fetch-kev-artifacts.sh --root /path/to/candidate-artifacts \
  --lock crates/kev/fixtures/variants/kev-4b-c4bfa11/artifact-lock.json
```

Use a candidate root separate from a production bundle: bundle serving
scans every model directory under its root. Acquisition is not admission.
The [evaluation record](measurements/2026-09-20-candidate-4b.md) states the
per-family results and why this does not change Coder's default.
