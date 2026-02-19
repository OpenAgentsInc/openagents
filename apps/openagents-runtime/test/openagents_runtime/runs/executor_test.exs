defmodule OpenAgentsRuntime.Runs.ExecutorTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Cancel
  alias OpenAgentsRuntime.Runs.Executor
  alias OpenAgentsRuntime.Runs.Frames
  alias OpenAgentsRuntime.Runs.Leases
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents

  test "consumes frames in order and reaches deterministic succeeded terminal state" do
    run_id = unique_run_id("exec_order")
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
               type: "user_message",
               payload: %{"text" => "world"}
             })

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "frame_3",
               type: "complete",
               payload: %{}
             })

    assert {:ok, %{processed_frames: 3, status: "succeeded", terminal_reason_class: "completed"}} =
             Executor.run_once(run_id, lease_owner: "worker-order")

    events = RunEvents.list_after(run_id, 0)

    assert Enum.map(events, & &1.event_type) == [
             "run.started",
             "run.delta",
             "run.delta",
             "run.finished"
           ]

    [_, first_delta, second_delta, finish_event] = events
    assert first_delta.payload["delta"] == "hello"
    assert second_delta.payload["delta"] == "world"
    assert finish_event.payload["status"] == "succeeded"

    run = Repo.get!(Run, run_id)
    assert run.status == "succeeded"
    assert run.terminal_reason_class == "completed"
    assert is_integer(run.last_processed_frame_id)
    assert run.last_processed_frame_id > 0
    assert %DateTime{} = run.terminal_at
  end

  test "enforces single active executor by lease ownership" do
    run_id = unique_run_id("exec_lease")
    insert_run(run_id)

    now = DateTime.utc_now()
    assert {:ok, _lease} = Leases.acquire(run_id, "worker-a", now: now, ttl_seconds: 60)

    assert {:error, :lease_held} =
             Executor.run_once(run_id, lease_owner: "worker-b", now: now)
  end

  test "persists deterministic failed terminal reason class" do
    run_id = unique_run_id("exec_fail")
    insert_run(run_id)

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "frame_fail",
               type: "fail",
               payload: %{"reason_class" => "tool_timeout", "reason" => "tool call timed out"}
             })

    assert {:ok, %{processed_frames: 1, status: "failed", terminal_reason_class: "tool_timeout"}} =
             Executor.run_once(run_id, lease_owner: "worker-fail")

    run = Repo.get!(Run, run_id)
    assert run.status == "failed"
    assert run.terminal_reason_class == "tool_timeout"
    assert run.terminal_reason == "tool call timed out"

    [_, finish_event] = RunEvents.list_after(run_id, 0)
    assert finish_event.event_type == "run.finished"
    assert finish_event.payload["status"] == "failed"
    assert finish_event.payload["reason_class"] == "tool_timeout"
  end

  test "cancel requested is durable and prevents new work from starting" do
    run_id = unique_run_id("exec_cancel")
    insert_run(run_id)

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "frame_1",
               type: "user_message",
               payload: %{"text" => "should not run"}
             })

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "frame_2",
               type: "complete",
               payload: %{}
             })

    assert {:ok, %{idempotent_replay: false, status: "canceling"}} =
             Cancel.request_cancel(run_id, %{"reason" => "operator cancel"})

    assert {:ok,
            %{processed_frames: 0, status: "canceled", terminal_reason_class: "cancel_requested"}} =
             Executor.run_once(run_id, lease_owner: "worker-cancel")

    events = RunEvents.list_after(run_id, 0)
    assert Enum.any?(events, &(&1.event_type == "run.cancel_requested"))

    assert Enum.any?(
             events,
             &(&1.event_type == "run.finished" and &1.payload["status"] == "canceled")
           )

    refute Enum.any?(events, &(&1.event_type == "run.delta"))

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "frame_late",
               type: "user_message",
               payload: %{"text" => "late"}
             })

    assert {:ok,
            %{processed_frames: 0, status: "canceled", terminal_reason_class: "cancel_requested"}} =
             Executor.run_once(run_id, lease_owner: "worker-cancel")

    run = Repo.get!(Run, run_id)
    assert run.status == "canceled"
  end

  test "loop detection transitions run into deterministic failed terminal state" do
    run_id = unique_run_id("exec_loop")
    insert_run(run_id)

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "loop_1",
               type: "user_message",
               payload: %{"text" => "repeat"}
             })

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "loop_2",
               type: "user_message",
               payload: %{"text" => "repeat"}
             })

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "loop_3",
               type: "user_message",
               payload: %{"text" => "repeat"}
             })

    assert {:ok, %{status: "failed", terminal_reason_class: "loop_detected"}} =
             Executor.run_once(run_id,
               lease_owner: "worker-loop",
               loop_detection: [
                 warning_threshold: 3,
                 critical_threshold: 4,
                 global_circuit_breaker_threshold: 10
               ]
             )

    events = RunEvents.list_after(run_id, 0)

    assert Enum.any?(
             events,
             &(&1.event_type == "run.loop_detected" and
                 &1.payload["reason_code"] == "loop_detected.no_progress")
           )

    assert Enum.any?(
             events,
             &(&1.event_type == "run.finished" and &1.payload["reason_class"] == "loop_detected")
           )

    run = Repo.get!(Run, run_id)
    assert run.status == "failed"
    assert run.terminal_reason_class == "loop_detected"
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
end
