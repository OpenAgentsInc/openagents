defmodule OpenAgentsRuntime.Runs.RunEventsTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents

  setup do
    Repo.insert!(%Run{
      run_id: "run_events_1",
      thread_id: "thread_events_1",
      status: "running",
      owner_user_id: 10,
      latest_seq: 0
    })

    :ok
  end

  test "appends events with monotonic sequence numbers" do
    assert {:ok, event_1} = RunEvents.append_event("run_events_1", "run.started", %{"step" => 1})
    assert {:ok, event_2} = RunEvents.append_event("run_events_1", "run.progress", %{"step" => 2})

    assert event_1.seq == 1
    assert event_2.seq == 2
    assert RunEvents.latest_seq("run_events_1") == 2
  end

  test "returns run_not_found when run does not exist" do
    assert {:error, :run_not_found} = RunEvents.append_event("missing_run", "run.started", %{})
  end

  test "concurrent appends keep unique run-local ordering" do
    events =
      1..20
      |> Task.async_stream(
        fn idx -> RunEvents.append_event("run_events_1", "run.delta", %{"idx" => idx}) end,
        ordered: false,
        max_concurrency: 10,
        timeout: 5_000
      )
      |> Enum.map(fn {:ok, {:ok, event}} -> event end)

    seqs = events |> Enum.map(& &1.seq) |> Enum.sort()

    assert seqs == Enum.to_list(1..20)
    assert RunEvents.latest_seq("run_events_1") == 20
  end

  test "list_after returns sorted tail events" do
    assert {:ok, _} = RunEvents.append_event("run_events_1", "event.1", %{})
    assert {:ok, _} = RunEvents.append_event("run_events_1", "event.2", %{})
    assert {:ok, _} = RunEvents.append_event("run_events_1", "event.3", %{})

    assert [2, 3] ==
             RunEvents.list_after("run_events_1", 1)
             |> Enum.map(& &1.seq)
  end
end
