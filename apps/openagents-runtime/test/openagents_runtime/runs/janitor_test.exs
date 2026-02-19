defmodule OpenAgentsRuntime.Runs.JanitorTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.AgentRegistry
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Janitor
  alias OpenAgentsRuntime.Runs.Leases
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents

  @failure_reason_class "executor_recovery_exhausted"

  test "reconciles stale leases and resumes run via normal executor path" do
    run_id = unique_run_id("janitor_resume")
    insert_run(run_id)

    now = DateTime.utc_now()
    assert {:ok, _lease} = Leases.acquire(run_id, "worker-lost", now: now, ttl_seconds: 1)

    summary =
      Janitor.run_once(
        now: DateTime.add(now, 5, :second),
        max_recovery_attempts: 3,
        recovery_cooldown_ms: 0
      )

    assert summary.scanned == 1
    assert summary.resumed == 1

    run = Repo.get!(Run, run_id)
    assert run.recovery_attempt_count == 1
    assert %DateTime{} = run.last_recovery_at

    assert Enum.any?(RunEvents.list_after(run_id, 0), &(&1.event_type == "run.executor_lost"))
    assert_eventually(fn -> is_pid(AgentRegistry.whereis(run_id)) end)
  end

  test "marks run failed when stale executor recovery budget is exceeded" do
    run_id = unique_run_id("janitor_fail")
    insert_run(run_id, %{recovery_attempt_count: 2})

    now = DateTime.utc_now()
    assert {:ok, _lease} = Leases.acquire(run_id, "worker-lost", now: now, ttl_seconds: 1)

    summary =
      Janitor.run_once(
        now: DateTime.add(now, 5, :second),
        max_recovery_attempts: 2,
        recovery_cooldown_ms: 0
      )

    assert summary.scanned == 1
    assert summary.failed == 1

    run = Repo.get!(Run, run_id)
    assert run.status == "failed"
    assert run.terminal_reason_class == @failure_reason_class
    assert run.terminal_reason == "janitor recovery attempts exceeded"
    assert %DateTime{} = run.terminal_at

    events = RunEvents.list_after(run_id, 0)
    assert Enum.any?(events, &(&1.event_type == "run.executor_lost"))

    assert Enum.any?(events, fn event ->
             event.event_type == "run.finished" and
               event.payload["reason_class"] == @failure_reason_class
           end)
  end

  defp insert_run(run_id, overrides \\ %{}) do
    attrs =
      Map.merge(
        %{
          run_id: run_id,
          thread_id: "thread_#{run_id}",
          status: "running",
          owner_user_id: 7,
          latest_seq: 0,
          recovery_attempt_count: 0
        },
        overrides
      )

    Repo.insert!(struct(Run, attrs))
  end

  defp unique_run_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end

  defp assert_eventually(fun, attempts \\ 40, sleep_ms \\ 25)

  defp assert_eventually(fun, attempts, _sleep_ms) when attempts <= 0 do
    assert fun.()
  end

  defp assert_eventually(fun, attempts, sleep_ms) do
    if fun.() do
      :ok
    else
      Process.sleep(sleep_ms)
      assert_eventually(fun, attempts - 1, sleep_ms)
    end
  end
end
