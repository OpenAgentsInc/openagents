defmodule OpenAgentsRuntime.Convex.ProjectorTest do
  use OpenAgentsRuntime.DataCase, async: false

  import ExUnit.CaptureLog

  alias OpenAgentsRuntime.Codex.Workers
  alias OpenAgentsRuntime.Convex.Projector
  alias OpenAgentsRuntime.Convex.ProjectionCheckpoint
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents

  @fixed_now ~U[2026-02-19 20:00:00.000000Z]
  @projection_version "convex_summary_v1"

  test "project_run/2 writes deterministic run summary projection from runtime events" do
    run_id = unique_id("run_projection")
    thread_id = unique_id("thread_projection")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "running",
      owner_user_id: 42,
      latest_seq: 0
    })

    {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})
    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "hello"})
    {:ok, _} = RunEvents.append_event(run_id, "run.finished", %{"status" => "succeeded"})
    clear_checkpoint("run_summary", run_id)

    assert {:ok, %{document_id: document_id, summary: summary, write: write}} =
             Projector.project_run(run_id,
               sink: __MODULE__.CaptureSink,
               sink_opts: [test_pid: self()],
               now: @fixed_now,
               projection_version: @projection_version
             )

    assert document_id == "runtime/run_summary:#{run_id}"
    assert_receive {:run_summary_upserted, ^document_id, ^summary}

    assert summary["kind"] == "run_summary"
    assert summary["run_id"] == run_id
    assert summary["thread_id"] == thread_id
    assert summary["status"] == "running"
    assert summary["latest_seq"] == 3
    assert summary["event_count"] == 3
    assert summary["latest_event_type"] == "run.finished"
    assert summary["runtime_source"] == %{"entity" => "run", "run_id" => run_id, "seq" => 3}
    assert summary["projection_version"] == @projection_version
    assert summary["projected_at"] == DateTime.to_iso8601(@fixed_now)
    assert write == "applied"
  end

  test "project_codex_worker/2 writes deterministic worker summary projection from runtime events" do
    worker_id = unique_id("worker_projection")

    assert {:ok, %{worker: _worker}} =
             Workers.create_worker(
               %{
                 "worker_id" => worker_id,
                 "workspace_ref" => "workspace://project",
                 "metadata" => %{"suite" => "projector_test"}
               },
               %{user_id: 77}
             )

    assert {:ok, _response} =
             Workers.submit_request(worker_id, %{user_id: 77}, %{
               "request_id" => "req_projection_1",
               "method" => "thread/start",
               "params" => %{"prompt" => "hello"}
             })

    assert {:ok, _stop_result} = Workers.stop_worker(worker_id, %{user_id: 77}, reason: "done")
    clear_checkpoint("codex_worker_summary", worker_id)

    assert {:ok, %{document_id: document_id, summary: summary, write: write}} =
             Projector.project_codex_worker(worker_id,
               sink: __MODULE__.CaptureSink,
               sink_opts: [test_pid: self()],
               now: @fixed_now,
               projection_version: @projection_version
             )

    assert document_id == "runtime/codex_worker_summary:#{worker_id}"
    assert_receive {:codex_worker_summary_upserted, ^document_id, ^summary}

    assert summary["kind"] == "codex_worker_summary"
    assert summary["worker_id"] == worker_id
    assert summary["status"] == "stopped"
    assert summary["latest_seq"] == 4
    assert summary["event_count"] == 4
    assert summary["latest_event_type"] == "worker.stopped"

    assert summary["runtime_source"] == %{
             "entity" => "codex_worker",
             "worker_id" => worker_id,
             "seq" => 4
           }

    assert summary["projection_version"] == @projection_version
    assert summary["projected_at"] == DateTime.to_iso8601(@fixed_now)
    assert write == "applied"
  end

  test "project_run/2 skips sink writes for idempotent replay of same sequence range" do
    run_id = unique_id("run_projection_idempotent")
    thread_id = unique_id("thread_projection_idempotent")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "running",
      owner_user_id: 58,
      latest_seq: 0
    })

    {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})
    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "hello"})
    clear_checkpoint("run_summary", run_id)

    assert {:ok, %{document_id: document_id, write: "applied"}} =
             Projector.project_run(run_id,
               sink: __MODULE__.CaptureSink,
               sink_opts: [test_pid: self()],
               now: @fixed_now,
               projection_version: @projection_version
             )

    assert_receive {:run_summary_upserted, ^document_id, _summary}

    assert {:ok, %{document_id: ^document_id, write: "skipped", reason: "idempotent"}} =
             Projector.project_run(run_id,
               sink: __MODULE__.CaptureSink,
               sink_opts: [test_pid: self()],
               now: DateTime.add(@fixed_now, 10, :second),
               projection_version: @projection_version
             )

    refute_receive {:run_summary_upserted, ^document_id, _summary}

    checkpoint = checkpoint!("run_summary", run_id)
    assert checkpoint.last_runtime_seq == 2
    assert checkpoint.projection_version == @projection_version
    assert String.length(checkpoint.summary_hash) == 64
  end

  test "project_run/2 emits drift signal and rewrites projection when checkpoint hash diverges" do
    run_id = unique_id("run_projection_drift")
    thread_id = unique_id("thread_projection_drift")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "running",
      owner_user_id: 73,
      latest_seq: 0
    })

    {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})
    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "hello drift"})
    clear_checkpoint("run_summary", run_id)

    assert {:ok, %{document_id: document_id, write: "applied"}} =
             Projector.project_run(run_id,
               sink: __MODULE__.CaptureSink,
               sink_opts: [test_pid: self()],
               now: @fixed_now,
               projection_version: @projection_version
             )

    assert_receive {:run_summary_upserted, ^document_id, _summary}

    checkpoint!("run_summary", run_id)
    |> Ecto.Changeset.change(summary_hash: String.duplicate("0", 64))
    |> Repo.update!()

    drift_ref = "convex-projector-drift-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        drift_ref,
        [:openagents_runtime, :convex, :projection, :drift],
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:projection_drift_telemetry, measurements, metadata})
        end,
        self()
      )

    on_exit(fn -> :telemetry.detach(drift_ref) end)

    assert {:ok, %{document_id: ^document_id, write: "applied", reason: "summary_hash_mismatch"}} =
             Projector.project_run(run_id,
               sink: __MODULE__.CaptureSink,
               sink_opts: [test_pid: self()],
               now: DateTime.add(@fixed_now, 12, :second),
               projection_version: @projection_version
             )

    assert_receive {:run_summary_upserted, ^document_id, _summary}
    assert_receive {:projection_drift_telemetry, measurements, metadata}
    assert measurements.count == 1
    assert metadata.projection == "run_summary"
    assert metadata.reason_class == "summary_hash_mismatch"
  end

  test "project_run/2 surfaces sink write failure via logs and telemetry" do
    run_id = unique_id("run_projection_fail")
    thread_id = unique_id("thread_projection_fail")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "running",
      owner_user_id: 91,
      latest_seq: 0
    })

    {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})
    clear_checkpoint("run_summary", run_id)

    telemetry_ref = "convex-projector-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        telemetry_ref,
        [:openagents_runtime, :convex, :projection, :write],
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:projection_write_telemetry, measurements, metadata})
        end,
        self()
      )

    on_exit(fn -> :telemetry.detach(telemetry_ref) end)

    log =
      capture_log(fn ->
        assert {:error, {:sink_write_failed, :simulated_sink_failure}} =
                 Projector.project_run(run_id,
                   sink: __MODULE__.FailingSink,
                   now: @fixed_now,
                   projection_version: @projection_version
                 )
      end)

    assert log =~ "convex projection write failed"

    assert_receive {:projection_write_telemetry, measurements, metadata}
    assert measurements.count == 1
    assert is_integer(measurements.duration_ms)
    assert metadata.projection == "run_summary"
    assert metadata.result == "error"
    assert metadata.reason_class == "simulated_sink_failure"
  end

  defp checkpoint!(projection_name, entity_id) do
    Repo.get_by!(ProjectionCheckpoint, projection_name: projection_name, entity_id: entity_id)
  end

  defp clear_checkpoint(projection_name, entity_id) do
    from(checkpoint in ProjectionCheckpoint,
      where: checkpoint.projection_name == ^projection_name and checkpoint.entity_id == ^entity_id
    )
    |> Repo.delete_all()
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

  defmodule FailingSink do
    @behaviour OpenAgentsRuntime.Convex.Sink

    @impl true
    def upsert_run_summary(_document_id, _summary, _opts), do: {:error, :simulated_sink_failure}

    @impl true
    def upsert_codex_worker_summary(_document_id, _summary, _opts),
      do: {:error, :simulated_sink_failure}
  end
end
