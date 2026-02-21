defmodule OpenAgentsRuntime.Sync.ProjectorSinkTest do
  use OpenAgentsRuntime.DataCase, async: false

  import Ecto.Query

  alias OpenAgentsRuntime.Codex.Workers
  alias OpenAgentsRuntime.Khala.FanoutSink
  alias OpenAgentsRuntime.Khala.ProjectionCheckpoint
  alias OpenAgentsRuntime.Khala.Projector
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Sync.CodexWorkerSummary
  alias OpenAgentsRuntime.Sync.ProjectorSink
  alias OpenAgentsRuntime.Sync.RunSummary
  alias OpenAgentsRuntime.Sync.StreamEvent

  @fixed_now ~U[2026-02-20 00:00:00.000000Z]
  @run_topic "runtime.run_summaries"
  @worker_topic "runtime.codex_worker_summaries"

  test "project_run writes read model and stream event in inline mode" do
    run_id = unique_id("run_sync")
    thread_id = unique_id("thread_sync")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "running",
      owner_user_id: 10,
      latest_seq: 0
    })

    {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})
    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "hello"})
    clear_checkpoint("run_summary", run_id)

    assert {:ok, %{document_id: document_id, write: "applied"}} =
             Projector.project_run(run_id,
               sink: ProjectorSink,
               now: @fixed_now,
               projection_version: "khala_summary_v1"
             )

    assert %RunSummary{} = read_model = Repo.get!(RunSummary, document_id)
    assert read_model.doc_version == 2
    assert read_model.payload["run_id"] == run_id
    assert is_binary(read_model.payload_hash)
    assert byte_size(read_model.payload_hash) == 32

    assert [%StreamEvent{} = stream_event] =
             from(event in StreamEvent,
               where: event.topic == @run_topic and event.doc_key == ^document_id,
               order_by: [asc: event.watermark]
             )
             |> Repo.all()

    assert stream_event.watermark == 1
    assert stream_event.doc_version == 2
    assert is_map(stream_event.payload)
    assert is_binary(stream_event.payload_hash)
    assert byte_size(stream_event.payload_hash) == 32
  end

  test "project_run supports pointer mode stream rows" do
    run_id = unique_id("run_pointer")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: unique_id("thread_pointer"),
      status: "running",
      owner_user_id: 11,
      latest_seq: 0
    })

    {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})
    clear_checkpoint("run_summary", run_id)

    assert {:ok, %{document_id: document_id, write: "applied"}} =
             Projector.project_run(run_id,
               sink: ProjectorSink,
               sink_opts: [stream_payload_mode: :pointer],
               now: @fixed_now,
               projection_version: "khala_summary_v1"
             )

    assert %RunSummary{} = read_model = Repo.get!(RunSummary, document_id)
    assert is_map(read_model.payload)

    assert [%StreamEvent{} = stream_event] =
             from(event in StreamEvent,
               where: event.topic == @run_topic and event.doc_key == ^document_id
             )
             |> Repo.all()

    assert stream_event.payload == nil
  end

  test "project_codex_worker writes codex worker read model and stream event" do
    worker_id = unique_id("worker_sync")

    assert {:ok, %{worker: _worker}} =
             Workers.create_worker(
               %{
                 "worker_id" => worker_id,
                 "workspace_ref" => "workspace://project",
                 "metadata" => %{"suite" => "sync_projector_sink_test"}
               },
               %{user_id: 77}
             )

    assert {:ok, _response} =
             Workers.submit_request(worker_id, %{user_id: 77}, %{
               "request_id" => "req_1",
               "method" => "thread/start",
               "params" => %{"prompt" => "hello"}
             })

    assert {:ok, _stop_result} = Workers.stop_worker(worker_id, %{user_id: 77}, reason: "done")

    clear_checkpoint("codex_worker_summary", worker_id)

    assert {:ok, %{document_id: document_id, write: "applied"}} =
             Projector.project_codex_worker(worker_id,
               sink: ProjectorSink,
               now: @fixed_now,
               projection_version: "khala_summary_v1"
             )

    assert %CodexWorkerSummary{} = read_model = Repo.get!(CodexWorkerSummary, document_id)
    assert read_model.doc_version == 4
    assert read_model.payload["worker_id"] == worker_id

    assert [%StreamEvent{} = stream_event] =
             from(event in StreamEvent,
               where: event.topic == @worker_topic and event.doc_key == ^document_id,
               order_by: [asc: event.watermark]
             )
             |> Repo.all()

    assert stream_event.watermark == 1
    assert stream_event.doc_version == 4
  end

  test "fanout sink supports dual publish with capture sink and khala sink" do
    run_id = unique_id("run_fanout")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: unique_id("thread_fanout"),
      status: "running",
      owner_user_id: 99,
      latest_seq: 0
    })

    {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})
    clear_checkpoint("run_summary", run_id)

    assert {:ok, %{document_id: document_id, write: "applied"}} =
             Projector.project_run(run_id,
               sink: FanoutSink,
               sink_opts: [
                 sinks: [__MODULE__.CaptureSink, ProjectorSink],
                 test_pid: self()
               ],
               now: @fixed_now,
               projection_version: "khala_summary_v1"
             )

    assert_receive {:capture_sink_upserted, ^document_id}
    assert %RunSummary{} = Repo.get!(RunSummary, document_id)
  end

  defp clear_checkpoint(projection_name, entity_id) do
    from(checkpoint in ProjectionCheckpoint,
      where: checkpoint.projection_name == ^projection_name and checkpoint.entity_id == ^entity_id
    )
    |> Repo.delete_all()
  end

  defp unique_id(prefix), do: "#{prefix}_#{System.unique_integer([:positive])}"

  defmodule CaptureSink do
    @behaviour OpenAgentsRuntime.Khala.Sink

    @impl true
    def upsert_run_summary(document_id, _summary, opts) do
      send(Keyword.fetch!(opts, :test_pid), {:capture_sink_upserted, document_id})
      :ok
    end

    @impl true
    def upsert_codex_worker_summary(_document_id, _summary, _opts), do: :ok
  end
end
