# gym run — issue #168 work notes

## What landed

- New `crates/openagents-cli/src/gym/run.rs`
  - `GymClient` for the lifecycle routes: `POST /api/v1/gym/runs/start`, `POST /api/v1/gym/runs/{id}/trials`, `PATCH /api/v1/gym/runs/{id}`, plus `GET /api/v1/gym/runs/{id}` and list/cancel wiring.
  - `resolve_for_run` (via `suite.rs`) refuses drifted suites before any network call.
  - `--dry-run` resolves the suite, prints the exact lifecycle calls, and exits without registering or executing.
  - Host-native Harbor path behind a single `run_on_host` function; container/Box targets are the next lane.
  - `finalize_job_dir` enforces the crashed-verifier rule: a run whose verifier never graded a single trial is patched `abandoned`, never `graded`.
  - `openagents.gym.run_status.v1` emitted via `RunStatus` from `gym/schemas.rs`.
  - Lane and catalog model inference matches `bench/run-suite.sh` (`ollama/` -> `local`, otherwise `proxy`; `ollama:<name>` vs provider catalog id).
- `crates/openagents-cli/src/gym/suite.rs`
  - Added `ResolvedSuite` / `ResolvedTask` and `resolve_for_run` / `resolve_for_run_in` for `gym run` to consume the existing #167 manifest/digest code.
- `crates/openagents-cli/src/gym/schemas.rs`
  - Added `RUN_STATUS_SCHEMA` constant.
- `crates/openagents-cli/src/cli.rs`
  - Added `GymAction::Run` and wired it through `run_gym` with `api_base` and token.
- `crates/openagents-cli/tests/gym_run_test.rs`
  - Stub-server tests for the lifecycle wire assertions, drift refusal, dry-run, and golden `run_status.v1` shape.

## Tests per acceptance criterion

- `gym run <suite-id> --model ...` completes end to end: host path implemented; the actual Harbor binary call is the remaining runtime dependency. Covered by `dry_run_resolves_suite_and_does_not_contact_api` and the lifecycle unit tests.
- Crashed verifier -> `abandoned`: `finalize_patches_abandoned_when_no_verifier_graded`.
- Pin drift refuses before registration: `drifted_suite_refuses_before_any_network_call`.
- `--dry-run` registers nothing: `dry_run_resolves_suite_and_does_not_contact_api` (passes a closed stub port and still resolves/prints).
- Stub-server wire assertions: `start_run_posts_the_expected_payload_and_auth`, `finalize_upserts_graded_trials_and_patches_graded`.
- Golden `run_status.v1` shape: `run_status_json_matches_golden_shape`.
- Lane/model inference: `lane_and_catalog_model_match_shell_inference`.

## Gaps

- Container and Box execution targets are not implemented; the `HostPlan`/`run_on_host` structure is meant for them to plug in.
- `gym run status/list/cancel` are wired through `GymClient` but have not been exercised against a real server.
- The `--env <provider>` flag is passed through to the printed/constructed Harbor args but host Harbor support for it is not validated.
- `gym run cancel` calls `PATCH /api/v1/gym/runs/{id}` with `{"status":"cancelled"}`; if the server later adds a dedicated route, the call should be updated.
- The `harbor_runner_image` and `harbor_runner_image` checks in `env.rs` are `not_yet_built` because the B3 harbor-runner image has not landed; `gym run` on the host is the fallback.

## Gate status

- `cargo test -p openagents-cli`: **green** (all openagents-cli tests passed).
- `cargo test -p openagents-cli --test gym_run_test`: **green** (7/7).
- `pnpm run check`: the Rust `fmt:check` component is green. The full run hit a pre-existing `delegate_test` timing flake (`count_is_real_concurrency_and_the_cap_is_real_too`) under full-workspace load. Re-running that test in isolation passed:
  - `cargo test -p openagents-cli --test delegate_test -- count_is_real_concurrency_and_the_cap_is_real_too` -> ok.
  The failure is not related to the `gym run` changes.
