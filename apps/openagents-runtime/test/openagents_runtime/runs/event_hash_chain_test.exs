defmodule OpenAgentsRuntime.Runs.EventHashChainTest do
  use OpenAgentsRuntime.DataCase, async: true

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.EventHashChain
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent
  alias OpenAgentsRuntime.Runs.RunEvents

  setup do
    run_id = "run_chain_#{System.unique_integer([:positive])}"

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: "thread_chain",
      status: "running",
      owner_user_id: 12,
      latest_seq: 0
    })

    %{run_id: run_id}
  end

  test "verifies intact hash chain", %{run_id: run_id} do
    assert {:ok, _} = RunEvents.append_event(run_id, "event.1", %{"x" => 1})
    assert {:ok, _} = RunEvents.append_event(run_id, "event.2", %{"x" => 2})
    assert {:ok, _} = RunEvents.append_event(run_id, "event.3", %{"x" => 3})

    assert {:ok, %{event_count: 3, head_hash: head_hash}} = EventHashChain.verify_run(run_id)
    assert is_binary(head_hash)
    assert String.length(head_hash) == 64
  end

  test "detects tampered event payload", %{run_id: run_id} do
    assert {:ok, _} = RunEvents.append_event(run_id, "event.1", %{"x" => 1})
    assert {:ok, _} = RunEvents.append_event(run_id, "event.2", %{"x" => 2})

    tampered_payload = %{"x" => 999}

    tamper_query =
      from(event in RunEvent,
        where: event.run_id == ^run_id and event.seq == 2,
        update: [set: [payload: ^tampered_payload]]
      )

    assert {1, _} = Repo.update_all(tamper_query, [])

    assert {:error, {:chain_broken, 2, "event_hash mismatch"}} = EventHashChain.verify_run(run_id)
  end
end
