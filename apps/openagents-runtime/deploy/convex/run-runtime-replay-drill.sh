#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command: $cmd" >&2
    exit 1
  fi
}

require_cmd mix
require_cmd awk

SEED_OUTPUT="$(
  cd "$RUNTIME_DIR"
  mix ecto.migrate >/dev/null
  mix run -e '
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Codex.Workers

  suffix = System.unique_integer([:positive])
  run_id = "g7_drill_run_#{suffix}"
  thread_id = "thread_#{run_id}"
  worker_id = "g7_drill_worker_#{suffix}"

  Repo.insert!(%Run{run_id: run_id, thread_id: thread_id, status: "running", owner_user_id: 9901, latest_seq: 0})
  {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})
  {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "g7 drill"})
  {:ok, _} = RunEvents.append_event(run_id, "run.finished", %{"status" => "succeeded", "reason_class" => "drill"})

  {:ok, %{worker: _worker}} = Workers.create_worker(%{"worker_id" => worker_id, "adapter" => "desktop_bridge"}, %{user_id: 9901})
  {:ok, _} = Workers.ingest_event(worker_id, %{user_id: 9901}, %{"event_type" => "worker.heartbeat", "payload" => %{"source" => "g7-drill"}})
  {:ok, _} = Workers.stop_worker(worker_id, %{user_id: 9901}, reason: "drill_done")

  IO.puts("RUN_ID=#{run_id}")
  IO.puts("WORKER_ID=#{worker_id}")
  '
)"

RUN_ID="$(printf '%s\n' "$SEED_OUTPUT" | awk -F= '/^RUN_ID=/{print $2}' | tail -n 1)"
WORKER_ID="$(printf '%s\n' "$SEED_OUTPUT" | awk -F= '/^WORKER_ID=/{print $2}' | tail -n 1)"

if [[ -z "$RUN_ID" || -z "$WORKER_ID" ]]; then
  echo "Failed to capture seeded drill IDs." >&2
  printf '%s\n' "$SEED_OUTPUT" >&2
  exit 1
fi

RUN_REPROJECT_OUTPUT="$(cd "$RUNTIME_DIR" && mix runtime.convex.reproject --run-id "$RUN_ID")"
WORKER_REPROJECT_OUTPUT="$(cd "$RUNTIME_DIR" && mix runtime.convex.reproject --worker-id "$WORKER_ID")"

echo "Runtime replay drill completed."
echo "Run ID:            $RUN_ID"
echo "Worker ID:         $WORKER_ID"
echo "Run reproject:     $(printf '%s\n' "$RUN_REPROJECT_OUTPUT" | tail -n 1)"
echo "Worker reproject:  $(printf '%s\n' "$WORKER_REPROJECT_OUTPUT" | tail -n 1)"
