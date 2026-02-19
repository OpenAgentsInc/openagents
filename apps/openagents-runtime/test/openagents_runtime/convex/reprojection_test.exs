defmodule OpenAgentsRuntime.Convex.ReprojectionTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Codex.Workers
  alias OpenAgentsRuntime.Convex.ProjectionCheckpoint
  alias OpenAgentsRuntime.Convex.Reprojection
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents

  @fixed_now ~U[2026-02-19 21:00:00.000000Z]
  @projection_version "convex_summary_v1"

  test "rebuild_run/2 drops checkpoint and replays deterministic projection" do
    run_id = unique_id("run_rebuild")
    thread_id = unique_id("thread_rebuild")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "running",
      owner_user_id: 44,
      latest_seq: 0
    })

    {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})
    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "hello rebuild"})

    checkpoint = checkpoint!("run_summary", run_id)

    checkpoint
    |> Ecto.Changeset.change(last_runtime_seq: 999, summary_hash: String.duplicate("0", 64))
    |> Repo.update!()

    telemetry_ref = "convex-reprojection-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        telemetry_ref,
        [:openagents_runtime, :convex, :projection, :replay],
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:projection_replay_telemetry, measurements, metadata})
        end,
        self()
      )

    on_exit(fn -> :telemetry.detach(telemetry_ref) end)

    assert {:ok, result} =
             Reprojection.rebuild_run(run_id,
               sink: __MODULE__.CaptureSink,
               sink_opts: [test_pid: self()],
               now: @fixed_now,
               projection_version: @projection_version
             )

    assert result.scope == "run"
    assert result.entity_id == run_id
    assert result.result == "ok"
    assert result.write == "applied"
    assert is_integer(result.duration_ms)

    assert_receive {:run_summary_upserted, document_id, summary}
    assert document_id == "runtime/run_summary:#{run_id}"
    assert summary["latest_seq"] == 2

    checkpoint = checkpoint!("run_summary", run_id)
    assert checkpoint.last_runtime_seq == 2

    assert_receive {:projection_replay_telemetry, measurements, metadata}
    assert measurements.count == 1
    assert is_integer(measurements.duration_ms)
    assert metadata.scope == "run"
    assert metadata.result == "ok"
  end

  test "rebuild_all/1 replays run and worker projections and returns summary" do
    run_id = unique_id("run_rebuild_all")
    thread_id = unique_id("thread_rebuild_all")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "running",
      owner_user_id: 51,
      latest_seq: 0
    })

    {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})

    worker_id = unique_id("worker_rebuild_all")

    assert {:ok, %{worker: _worker}} =
             Workers.create_worker(%{"worker_id" => worker_id}, %{user_id: 51})

    assert {:ok, summary} =
             Reprojection.rebuild_all(
               run_ids: [run_id],
               worker_ids: [worker_id],
               sink: __MODULE__.CaptureSink,
               sink_opts: [test_pid: self()],
               now: @fixed_now,
               projection_version: @projection_version
             )

    assert summary.total_entities == 2
    assert summary.succeeded_entities == 2
    assert summary.failed_entities == 0

    run_result = Enum.find(summary.results, &(&1.scope == "run" and &1.entity_id == run_id))

    worker_result =
      Enum.find(summary.results, &(&1.scope == "codex_worker" and &1.entity_id == worker_id))

    assert run_result
    assert run_result.result == "ok"
    assert worker_result
    assert worker_result.result == "ok"

    assert_receive {:run_summary_upserted, "runtime/run_summary:" <> _run_id, _summary}

    assert_receive {:codex_worker_summary_upserted, "runtime/codex_worker_summary:" <> _worker_id,
                    _summary}
  end

  test "rebuild_codex_worker/2 converges runtime snapshot and convex summary after stop/resume replay" do
    worker_id = unique_id("worker_rebuild_resume")

    assert {:ok, %{worker: _worker}} =
             Workers.create_worker(%{"worker_id" => worker_id, "adapter" => "desktop_bridge"}, %{
               user_id: 62
             })

    assert {:ok, _} =
             Workers.ingest_event(worker_id, %{user_id: 62}, %{
               "event_type" => "worker.heartbeat",
               "payload" => %{"source" => "desktop"}
             })

    assert {:ok, _} = Workers.stop_worker(worker_id, %{user_id: 62}, reason: "desktop_exit")

    assert {:ok, %{worker: _worker, idempotent_replay: false}} =
             Workers.create_worker(%{"worker_id" => worker_id, "adapter" => "desktop_bridge"}, %{
               user_id: 62
             })

    assert {:ok, _} =
             Workers.ingest_event(worker_id, %{user_id: 62}, %{
               "event_type" => "worker.heartbeat",
               "payload" => %{"source" => "desktop", "kind" => "resume"}
             })

    assert {:ok, snapshot} = Workers.snapshot(worker_id, %{user_id: 62})

    assert {:ok, result} =
             Reprojection.rebuild_codex_worker(worker_id,
               sink: __MODULE__.CaptureSink,
               sink_opts: [test_pid: self()],
               now: @fixed_now,
               projection_version: @projection_version
             )

    assert result.scope == "codex_worker"
    assert result.entity_id == worker_id
    assert result.result == "ok"

    assert_receive {:codex_worker_summary_upserted, document_id, summary}
    assert document_id == "runtime/codex_worker_summary:#{worker_id}"
    assert summary["worker_id"] == worker_id
    assert summary["status"] == snapshot["status"]
    assert summary["latest_seq"] == snapshot["latest_seq"]
    assert summary["latest_event_type"] == "worker.heartbeat"

    checkpoint = checkpoint!("codex_worker_summary", worker_id)
    assert checkpoint.last_runtime_seq == snapshot["latest_seq"]
  end

  defp checkpoint!(projection_name, entity_id) do
    Repo.get_by!(ProjectionCheckpoint, projection_name: projection_name, entity_id: entity_id)
  end

  defp unique_id(prefix), do: "#{prefix}_#{System.unique_integer([:positive])}"

  defmodule CaptureSink do
    @behaviour OpenAgentsRuntime.Convex.Sink

    @impl true
    def upsert_run_summary(document_id, summary, opts) do
      send(Keyword.fetch!(opts, :test_pid), {:run_summary_upserted, document_id, summary})
      :ok
    end

    @impl true
    def upsert_codex_worker_summary(document_id, summary, opts) do
      send(
        Keyword.fetch!(opts, :test_pid),
        {:codex_worker_summary_upserted, document_id, summary}
      )

      :ok
    end
  end
end
