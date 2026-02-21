defmodule OpenAgentsRuntime.Memory.CompactionJobTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Memory.CompactionJob
  alias OpenAgentsRuntime.Memory.TimelineStore
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run

  setup do
    run_id = unique_run_id("compaction_job")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: "thread_#{run_id}",
      status: "running",
      owner_user_id: 88,
      latest_seq: 0
    })

    if pid = Process.whereis(CompactionJob) do
      Ecto.Adapters.SQL.Sandbox.allow(Repo, self(), pid)
    end

    %{run_id: run_id}
  end

  test "pressure trigger compacts run immediately", %{run_id: run_id} do
    for seq <- 1..2 do
      assert {:ok, _event} =
               TimelineStore.append_raw_event(run_id, %{
                 seq: seq,
                 event_type: "run.delta",
                 event_class: "default",
                 payload: %{"delta" => "job-#{seq}"}
               })
    end

    assert {:ok, result} = CompactionJob.trigger_pressure(run_id, event_class: "default")
    assert result.status == :succeeded

    [compaction | _] = TimelineStore.list_compactions(run_id)
    assert compaction.trigger_type == "pressure"
  end

  defp unique_run_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end
end
