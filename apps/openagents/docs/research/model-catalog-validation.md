# Model catalog validation

## Scope and behavior

The model RPC still returns a bare array. Catalog provenance (`live`, `cache`, or
`static`) and the selected binary path/version are recorded in tracing, keeping
existing clients compatible. `ListModels.force` is optional and defaults to false.

| Round one item | Result | Evidence |
| --- | --- | --- |
| 1. Binary selection | Fixed | `executable.rs` deduplicates canonical paths, compares semantic versions, caches by path/mtime/size, bounds probes to two seconds, and includes binary diagnostics. Tests cover newer candidates, replacement, ties, stderr, nonzero status, and termination. Shared by native/ACP resolvers. |
| 2. Codex response hygiene | Fixed; hidden filtering, pagination and default ordering retained | `current_schema_and_legacy_visibility_are_compatible` and `model_page_skips_hidden_and_unknown_efforts`; old responses warn with binary identity; upgrade ids become description suffixes. |
| 3. Last-good catalogs | Fixed | Shared `catalog.rs`; `tests/model_catalog.rs` exercises native/ACP retention, auth-context invalidation, and empty responses. Cursor uses the moved helper. Retries, overlap coalescing, force and cooldown are unit tested. |
| 4. Engine disk persistence | Fixed | Seven `model_catalogs` tests cover persistence, restart fallback, slow background refresh, credential races, corrupt files, revoked auth and missing binaries. Bare RPC shape retained; UI forwards force. |
| 5. Claude settings | Fixed | `settings_models_keep_manifest_and_full_ladder` uses a temporary settings file; config root honors `CLAUDE_CONFIG_DIR`. |
| 6. Picker resilience | Fixed | `refresh_failure_keeps_rows_and_catalog_change_reanchors_selection`; retained rows now accompany a refresh error strip and re-anchor after catalog replacement. |
| 7. OpenCode | Fixed within requested scope | Shared cache added without changing agent selection or directory-scoped discovery. `models_keep_large_catalog_on_empty_response_and_recover` covers a 512-model catalog. |

| Round two item | Result | Evidence |
| --- | --- | --- |
| 1. Claude live discovery | Fixed | `tests/claude_models.rs` verifies one shared initialize process for models and commands, curated metadata, concrete ids, live defaults, effort mapping, and failure/logged-out fallback. Initialize cache unit tests cover coalescing, two-minute expiry and context changes. |
| 2. Typed failures | Fixed | `catalog_failure.rs` and shared-cache tests cover timeout, failed, missing executable and auth required. Only transient failures serve stale rows. Engine also retires unauthorized disk catalogs; Claude retains its curated fallback. |
| 3. Empty catalogs | Fixed | Shared helper rejects empty catalogs; `codex_empty_catalogs_retire_children_and_next_request_spawns_fresh` verifies child retirement and recovery. Empty responses cannot replace successful catalogs. |
| 4. Selected-only tier | Fixed | `saved_model_survives_a_fresh_catalog_without_becoming_a_new_choice` and `explicit_missing_model_does_not_resolve_to_the_catalog_default` preserve the saved id/chip and show a selected-only row. The fresh catalog is not modified. |
| 5. Validation | See results below | Full crate tests, picker tests, clippy, diff check and live discovery. |

## Validation results

Commands use `CARGO_TARGET_DIR=/home/ubuntu/.cache/amber-otter-target`, with
`TMPDIR=/home/ubuntu/codex-runs/models-scratch` and
`ZERON_WORKTREES_DIR=/home/ubuntu/codex-runs/models-scratch/worktrees`.
Disk was checked before builds and remained above 71 GB free throughout the final
validation pass.

| Command | Result |
| --- | --- |
| `cargo test -p zeron-harness` | 351 passed, 11 ignored, 0 failed |
| `cargo test -p zeron-engine` | 416 passed, 13 ignored, 0 failed |
| `cargo test -p zeron-ui --lib pickers::` | 29 passed, 0 ignored, 0 failed; 1125 filtered out |
| `cargo clippy -p zeron-harness -p zeron-engine -p zeron-ui --all-targets --message-format=json` | Exit 0; no diagnostics intersect added lines. Existing unrelated warnings remain. |
| `git diff --check` | Clean |
| `cargo test -p zeron-harness --test cursor_shim repeated_startup_failures_retain_all_user_messages_without_nesting_or_duplicates -- --exact` | 1 passed; also passed in the final full suite (all 11 Cursor shim tests passed) |

The isolated Cursor failure from the disk-constrained run did not reproduce. The
solo run completed in 2.67 seconds, below its five-second timeout, and subsequent
full-suite runs passed. This is consistent with a contention-related flake.

Full validation also exposed two obsolete test expectations: the ACP fixture
counted the newly required version probe as catalog discovery, and ACP/OpenCode
refresh tests expected same-context failures to discard good catalogs. Those tests
now distinguish version probes and assert last-good retention plus recovery. The
OpenCode integration suite passes all 15 tests.

Logs are in `/home/ubuntu/codex-runs/models-scratch/`: `final-harness.log`,
`final-engine.log`, `final-ui.log`, `final-clippy.jsonl`, `final-clippy.log`,
`cursor-solo.log`, and `final-live-codex.log`.

### Live Codex discovery

`cargo run -p zeron-harness --example codex_models_probe` completed successfully:

```text
binary: /usr/local/lib/node_modules/@openai/codex/bin/codex.js
version: 0.153.4
source: live
gpt-6-astra
gpt-5.6-sol
gpt-5.6-terra
gpt-5.6-luna
gpt-daybreak-blue-latest
gpt-5.5
models: 6
```

### Implementation commits

- `c73f0780`: binary selection, configured Claude models, Codex schema hygiene.
- `c75eda4b`: portable version probes and diagnostics cleanup.
- `db081604`: shared contextual last-good catalogs.
- `52f4c978`: engine disk persistence and background refresh.
- `d635ddc1`: picker refresh resilience and force parameter.
- `df0e1d4a`: shared Claude initialize discovery.
- `ba1e2d1d`: typed failures and unauthorized stale-catalog retirement.
- `5a982aa2`: empty catalog rejection and Codex child retirement.
- `a0bd32b6`: selected-only picker rows and saved model identity.
- `9cbab9b5`: ACP refresh regression fixture and Claude discovery documentation.
- `fdc15eaf`: OpenCode empty-refresh retention regression.

## Limits

- Native Windows execution was not available on this Linux host. Portable path tests
  run here; the Windows launcher/version-probe test requires Windows CI.
- Existing unrelated clippy warnings remain outside this change.
- Cold discovery can incur a two-second version probe for each uncached candidate;
  later descriptor checks reuse the metadata-keyed probe result.
- Credential changes partition catalogs. Same-context disk rows may be displayed
  while a slow live refresh is pending; an auth failure retires that disk catalog.
- Live validation exercises the installed Codex binary. Other harness protocols are
  covered by fixtures, not authenticated live requests on this host.
