defmodule OpenAgentsRuntime.Codex.WorkersTest do
  use OpenAgentsRuntime.DataCase, async: false

  import Ecto.Query

  alias OpenAgentsRuntime.Codex.Worker
  alias OpenAgentsRuntime.Codex.Workers
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Sync.StreamEvent

  test "create_worker/3 submit_request/4 and stop_worker/3 lifecycle" do
    worker_id = unique_id("codexw")

    assert {:ok, %{worker: worker, idempotent_replay: false}} =
             Workers.create_worker(
               %{
                 "worker_id" => worker_id,
                 "workspace_ref" => "workspace://demo",
                 "metadata" => %{"suite" => "workers_test"}
               },
               %{user_id: 101}
             )

    assert worker.worker_id == worker_id
    assert worker.status == "running"

    assert {:ok, snapshot} = Workers.snapshot(worker_id, %{user_id: 101})
    assert snapshot["worker_id"] == worker_id
    assert snapshot["status"] == "running"
    assert snapshot["heartbeat_state"] == "fresh"
    assert is_integer(snapshot["heartbeat_stale_after_ms"])
    assert snapshot["heartbeat_stale_after_ms"] > 0

    assert {:ok, request_result} =
             Workers.submit_request(worker_id, %{user_id: 101}, %{
               "request_id" => "req_1",
               "method" => "thread/start",
               "params" => %{"prompt" => "hello"}
             })

    assert request_result["ok"] == true
    assert request_result["response"]["id"] == "req_1"

    events = Workers.list_after(worker_id, 0)
    event_types = Enum.map(events, & &1.event_type)

    assert "worker.started" in event_types
    assert "worker.request.received" in event_types
    assert "worker.response" in event_types

    sync_topic_events =
      from(stream_event in StreamEvent,
        where: stream_event.topic == "runtime.codex_worker_events",
        select: {stream_event.doc_key, stream_event.doc_version, stream_event.payload}
      )
      |> Repo.all()

    assert Enum.any?(sync_topic_events, fn {doc_key, _doc_version, payload} ->
             String.contains?(doc_key, worker_id) and
               payload["eventType"] == "worker.started"
           end)

    assert {:ok, stop_result} = Workers.stop_worker(worker_id, %{user_id: 101}, reason: "done")
    assert stop_result["status"] == "stopped"
    assert stop_result["idempotent_replay"] == false

    assert {:error, :worker_stopped} =
             Workers.submit_request(worker_id, %{user_id: 101}, %{"method" => "thread/start"})

    assert {:error, :worker_stopped} =
             Workers.ingest_event(worker_id, %{user_id: 101}, %{
               "event_type" => "worker.event",
               "payload" => %{"method" => "turn/completed"}
             })

    assert {:ok, %{worker: resumed_worker, idempotent_replay: false}} =
             Workers.create_worker(%{"worker_id" => worker_id, "adapter" => "desktop_bridge"}, %{
               user_id: 101
             })

    assert resumed_worker.worker_id == worker_id
    assert resumed_worker.status == "running"

    assert {:ok, resumed_request_result} =
             Workers.submit_request(worker_id, %{user_id: 101}, %{
               "request_id" => "req_2",
               "method" => "thread/start",
               "params" => %{"prompt" => "resume"}
             })

    assert resumed_request_result["ok"] == true

    assert {:ok, post_resume_stop} =
             Workers.stop_worker(worker_id, %{user_id: 101}, reason: "done")

    assert post_resume_stop["idempotent_replay"] == false

    assert {:ok, replay_stop} = Workers.stop_worker(worker_id, %{user_id: 101}, reason: "done")
    assert replay_stop["idempotent_replay"] == true

    events = Workers.list_after(worker_id, 0)
    assert Enum.count(events, &(&1.event_type == "worker.started")) >= 2
  end

  test "ownership enforcement denies cross-user access" do
    worker_id = unique_id("codexw")

    assert {:ok, _created} =
             Workers.create_worker(%{"worker_id" => worker_id}, %{user_id: 202})

    assert {:error, :forbidden} = Workers.snapshot(worker_id, %{user_id: 303})

    assert {:error, :forbidden} =
             Workers.submit_request(worker_id, %{user_id: 303}, %{"method" => "thread/start"})

    assert {:error, :forbidden} = Workers.stop_worker(worker_id, %{user_id: 303})
  end

  test "heartbeat stale detection is deterministic from configured threshold" do
    worker_id = unique_id("codexw")
    previous = Application.get_env(:openagents_runtime, :codex_worker_heartbeat_stale_after_ms)
    Application.put_env(:openagents_runtime, :codex_worker_heartbeat_stale_after_ms, 1_000)

    on_exit(fn ->
      if is_nil(previous) do
        Application.delete_env(:openagents_runtime, :codex_worker_heartbeat_stale_after_ms)
      else
        Application.put_env(:openagents_runtime, :codex_worker_heartbeat_stale_after_ms, previous)
      end
    end)

    assert {:ok, %{worker: _worker}} =
             Workers.create_worker(%{"worker_id" => worker_id}, %{user_id: 404})

    worker = Repo.get!(Worker, worker_id)
    stale_heartbeat_at = DateTime.add(worker.last_heartbeat_at, -2, :second)

    worker
    |> Ecto.Changeset.change(last_heartbeat_at: stale_heartbeat_at)
    |> Repo.update!()

    stale_now = DateTime.add(stale_heartbeat_at, 2_500, :millisecond)

    assert {:ok, snapshot} =
             Workers.snapshot(worker_id, %{user_id: 404}, now: stale_now)

    assert snapshot["heartbeat_state"] == "stale"
    assert snapshot["heartbeat_age_ms"] == 2500
    assert snapshot["heartbeat_stale_after_ms"] == 1000

    assert {:ok, workers} = Workers.list_workers(%{user_id: 404}, now: stale_now)
    assert [%{"worker_id" => ^worker_id, "heartbeat_state" => "stale"}] = workers
  end

  defp unique_id(prefix), do: "#{prefix}_#{System.unique_integer([:positive])}"
end
