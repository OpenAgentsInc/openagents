defmodule OpenAgentsRuntime.Runs.CancelTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Cancel
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents

  test "request_cancel is durable and idempotent" do
    run_id = unique_run_id("cancel")
    insert_run(run_id)

    assert {:ok, %{idempotent_replay: false, status: "canceling"}} =
             Cancel.request_cancel(run_id, %{"reason" => "user requested"})

    assert Cancel.cancel_requested?(run_id)
    assert Enum.any?(RunEvents.list_after(run_id, 0), &(&1.event_type == "run.cancel_requested"))

    assert {:ok, %{idempotent_replay: true, status: "canceling"}} =
             Cancel.request_cancel(run_id, %{"reason" => "user requested"})
  end

  test "request_cancel returns run_not_found for unknown runs" do
    assert {:error, :run_not_found} = Cancel.request_cancel("run_missing")
  end

  defp insert_run(run_id) do
    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: "thread_#{run_id}",
      status: "created",
      owner_user_id: 11,
      latest_seq: 0
    })
  end

  defp unique_run_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end
end
