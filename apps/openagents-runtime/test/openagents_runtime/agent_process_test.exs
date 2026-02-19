defmodule OpenAgentsRuntime.AgentProcessTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.AgentProcess
  alias OpenAgentsRuntime.AgentRegistry
  alias OpenAgentsRuntime.AgentSupervisor
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Frames
  alias OpenAgentsRuntime.Runs.Run

  test "routes frames through supervised agent process and executes run work" do
    run_id = unique_run_id("agent_route")
    insert_run(run_id)

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "frame_1",
               type: "user_message",
               payload: %{"text" => "hello"}
             })

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "frame_2",
               type: "complete",
               payload: %{}
             })

    assert :ok = AgentSupervisor.route_frame(run_id, "frame_1")

    assert_eventually(fn ->
      Repo.get!(Run, run_id).status == "succeeded"
    end)

    assert {:ok, state} = AgentProcess.snapshot(run_id)
    assert state.run_id == run_id
    assert state.frame_count >= 1
  end

  test "crashed run process restarts without impacting other supervised run processes" do
    run_a = unique_run_id("agent_restart_a")
    run_b = unique_run_id("agent_restart_b")
    insert_run(run_a)
    insert_run(run_b)

    assert {:ok, pid_a} = AgentSupervisor.ensure_agent(run_a)
    assert {:ok, pid_b} = AgentSupervisor.ensure_agent(run_b)
    assert Process.alive?(pid_a)
    assert Process.alive?(pid_b)

    Process.exit(pid_a, :kill)

    assert_eventually(fn ->
      case AgentRegistry.whereis(run_a) do
        pid when is_pid(pid) -> Process.alive?(pid) and pid != pid_a
        _ -> false
      end
    end)

    assert AgentRegistry.whereis(run_b) == pid_b
    assert Process.alive?(pid_b)
  end

  test "emits process-level mailbox and reduction instrumentation" do
    run_id = unique_run_id("agent_metrics")
    insert_run(run_id)

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "frame_metrics",
               type: "complete",
               payload: %{}
             })

    handler_id = "agent-process-test-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        handler_id,
        [:openagents_runtime, :agent_process, :stats],
        fn event_name, measurements, metadata, test_pid ->
          send(test_pid, {:agent_process_stats, event_name, measurements, metadata})
        end,
        self()
      )

    on_exit(fn -> :telemetry.detach(handler_id) end)

    assert :ok = AgentSupervisor.route_frame(run_id, "frame_metrics")

    assert_receive {:agent_process_stats, [:openagents_runtime, :agent_process, :stats],
                    measurements, metadata},
                   1_000

    assert is_integer(measurements.message_queue_len)
    assert is_integer(measurements.reductions)
    assert metadata.run_id == run_id
    assert metadata.event in [:ingest_frame, :execute, :executor_result, :executor_down]
  end

  defp insert_run(run_id) do
    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: "thread_#{run_id}",
      status: "created",
      owner_user_id: 42,
      latest_seq: 0
    })
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
