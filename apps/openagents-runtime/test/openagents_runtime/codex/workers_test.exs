defmodule OpenAgentsRuntime.Codex.WorkersTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Codex.Workers

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

    assert {:ok, stop_result} = Workers.stop_worker(worker_id, %{user_id: 101}, reason: "done")
    assert stop_result["status"] == "stopped"
    assert stop_result["idempotent_replay"] == false

    assert {:ok, replay_stop} = Workers.stop_worker(worker_id, %{user_id: 101}, reason: "done")
    assert replay_stop["idempotent_replay"] == true
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

  defp unique_id(prefix), do: "#{prefix}_#{System.unique_integer([:positive])}"
end
