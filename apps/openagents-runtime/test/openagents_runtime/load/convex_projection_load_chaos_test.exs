defmodule OpenAgentsRuntime.Load.ConvexProjectionLoadChaosTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Codex.Workers
  alias OpenAgentsRuntime.Convex.ProjectionCheckpoint
  alias OpenAgentsRuntime.Convex.Reprojection
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents

  @projection_write_event [:openagents_runtime, :convex, :projection, :write]
  @projection_write_failure_event [:openagents_runtime, :convex, :projection, :write_failure]

  @tag :load
  test "run projection checkpoint stays current during sustained event burst" do
    telemetry_ref = attach_projection_write_probe()
    on_exit(fn -> :telemetry.detach(telemetry_ref) end)

    run_id = unique_id("convex_run_load")
    thread_id = unique_id("convex_thread_load")
    total_events = 180

    insert_run!(run_id, thread_id, 9101)

    Enum.each(1..total_events, fn idx ->
      assert {:ok, _event} =
               RunEvents.append_event(run_id, "run.delta", %{"delta" => "load-#{idx}"})
    end)

    assert {:ok, _event} =
             RunEvents.append_event(run_id, "run.finished", %{
               "status" => "succeeded",
               "reason_class" => "load_test_done"
             })

    checkpoint = checkpoint!("run_summary", run_id)
    assert checkpoint.last_runtime_seq == total_events + 1

    lag_events = drain_projection_lag_events("run_summary")
    assert length(lag_events) >= total_events + 1
    assert Enum.max(lag_events) <= 1
  end

  @tag :load
  test "codex worker projection checkpoint stays current during heartbeat burst" do
    telemetry_ref = attach_projection_write_probe()
    on_exit(fn -> :telemetry.detach(telemetry_ref) end)

    worker_id = unique_id("convex_worker_load")
    heartbeat_events = 140
    principal = %{user_id: 9102}

    assert {:ok, %{worker: _worker}} =
             Workers.create_worker(
               %{"worker_id" => worker_id, "adapter" => "desktop_bridge"},
               principal
             )

    Enum.each(1..heartbeat_events, fn idx ->
      assert {:ok, _event} =
               Workers.ingest_event(worker_id, principal, %{
                 "event_type" => "worker.heartbeat",
                 "payload" => %{"seq" => idx}
               })
    end)

    assert {:ok, _stop} = Workers.stop_worker(worker_id, principal, reason: "load_done")
    assert {:ok, snapshot} = Workers.snapshot(worker_id, principal)

    checkpoint = checkpoint!("codex_worker_summary", worker_id)
    assert checkpoint.last_runtime_seq == snapshot["latest_seq"]

    lag_events = drain_projection_lag_events("codex_worker_summary")
    assert length(lag_events) >= heartbeat_events + 2
    assert Enum.max(lag_events) <= 1
  end

  @tag :load
  @tag :chaos
  test "sink failures do not block runtime writes and replay restores run projection" do
    telemetry_ref = attach_projection_write_probe()
    failure_ref = attach_projection_write_failure_probe()

    on_exit(fn ->
      :telemetry.detach(telemetry_ref)
      :telemetry.detach(failure_ref)
    end)

    previous_sink = Application.get_env(:openagents_runtime, :convex_projection_sink)
    Application.put_env(:openagents_runtime, :convex_projection_sink, __MODULE__.FailingSink)

    on_exit(fn ->
      Application.put_env(:openagents_runtime, :convex_projection_sink, previous_sink)
    end)

    run_id = unique_id("convex_run_chaos")
    thread_id = unique_id("convex_thread_chaos")
    total_events = 60

    insert_run!(run_id, thread_id, 9103)

    Enum.each(1..total_events, fn idx ->
      assert {:ok, _event} =
               RunEvents.append_event(run_id, "run.delta", %{"delta" => "chaos-#{idx}"})
    end)

    assert {:ok, _event} =
             RunEvents.append_event(run_id, "run.finished", %{
               "status" => "failed",
               "reason_class" => "chaos_sink_failure"
             })

    assert Repo.get!(Run, run_id).latest_seq == total_events + 1
    refute Repo.get_by(ProjectionCheckpoint, projection_name: "run_summary", entity_id: run_id)

    writes = drain_projection_writes("run_summary")
    assert length(writes) >= total_events + 1
    assert Enum.all?(writes, fn {_measurements, metadata} -> metadata.result == "error" end)

    failures = drain_projection_write_failures("run_summary")
    assert length(failures) >= total_events + 1

    assert {:ok, replay} =
             Reprojection.rebuild_run(run_id,
               sink: __MODULE__.CaptureSink,
               sink_opts: [test_pid: self()]
             )

    assert replay.scope == "run"
    assert replay.result == "ok"
    assert replay.write == "applied"

    checkpoint = checkpoint!("run_summary", run_id)
    assert checkpoint.last_runtime_seq == total_events + 1

    assert_receive {:run_summary_upserted, document_id, summary}, 1_000
    assert document_id == "runtime/run_summary:#{run_id}"
    assert summary["latest_seq"] == total_events + 1
  end

  defp insert_run!(run_id, thread_id, owner_user_id) do
    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "running",
      owner_user_id: owner_user_id,
      latest_seq: 0
    })
  end

  defp checkpoint!(projection_name, entity_id) do
    Repo.get_by!(ProjectionCheckpoint, projection_name: projection_name, entity_id: entity_id)
  end

  defp unique_id(prefix), do: "#{prefix}_#{System.unique_integer([:positive])}"

  defp attach_projection_write_probe do
    ref = "convex-load-write-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        ref,
        @projection_write_event,
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:projection_write, measurements, metadata})
        end,
        self()
      )

    ref
  end

  defp attach_projection_write_failure_probe do
    ref = "convex-load-write-failure-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        ref,
        @projection_write_failure_event,
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:projection_write_failure, measurements, metadata})
        end,
        self()
      )

    ref
  end

  defp drain_projection_lag_events(projection) do
    projection
    |> drain_projection_writes()
    |> Enum.map(fn {measurements, _metadata} ->
      measurements
      |> Map.get(:lag_events, 0)
      |> max(0)
    end)
  end

  defp drain_projection_writes(projection, acc \\ []) do
    receive do
      {:projection_write, measurements, %{projection: ^projection} = metadata} ->
        drain_projection_writes(projection, [{measurements, metadata} | acc])

      {:projection_write, _measurements, _metadata} ->
        drain_projection_writes(projection, acc)
    after
      150 ->
        Enum.reverse(acc)
    end
  end

  defp drain_projection_write_failures(projection, acc \\ []) do
    receive do
      {:projection_write_failure, measurements, %{projection: ^projection} = metadata} ->
        drain_projection_write_failures(projection, [{measurements, metadata} | acc])

      {:projection_write_failure, _measurements, _metadata} ->
        drain_projection_write_failures(projection, acc)
    after
      150 ->
        Enum.reverse(acc)
    end
  end

  defmodule FailingSink do
    @behaviour OpenAgentsRuntime.Convex.Sink

    @impl true
    def upsert_run_summary(_document_id, _summary, _opts), do: {:error, :chaos_sink_down}

    @impl true
    def upsert_codex_worker_summary(_document_id, _summary, _opts), do: {:error, :chaos_sink_down}
  end

  defmodule CaptureSink do
    @behaviour OpenAgentsRuntime.Convex.Sink

    @impl true
    def upsert_run_summary(document_id, summary, opts) do
      send(Keyword.fetch!(opts, :test_pid), {:run_summary_upserted, document_id, summary})
      :ok
    end

    @impl true
    def upsert_codex_worker_summary(_document_id, _summary, _opts), do: :ok
  end
end
