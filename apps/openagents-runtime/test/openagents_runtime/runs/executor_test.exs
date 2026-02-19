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
    handler_id = "parity-loop-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        handler_id,
        [:openagents_runtime, :parity, :failure],
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:parity_failure, measurements, metadata})
        end,
        self()
      )

    on_exit(fn -> :telemetry.detach(handler_id) end)

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

    assert_receive {:parity_failure, %{count: 1}, metadata}, 1_000
    assert metadata.class == "loop"
    assert metadata.reason_class == "loop_detected.no_progress"
    assert metadata.component == "runs.executor"
  end

  test "applies hook lifecycle in deterministic order and keeps effects receipt-visible" do
    run_id = unique_run_id("exec_hooks")
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

    hooks = [
      %{
        id: "model_high",
        hook: :before_model_resolve,
        priority: 30,
        handler: fn _event, _context ->
          %{"model_override" => "claude-sonnet", "provider_override" => "anthropic"}
        end
      },
      %{
        id: "prompt_high",
        hook: :before_prompt_build,
        priority: 20,
        handler: fn _event, _context ->
          %{"system_prompt" => "high", "prepend_context" => "[high]"}
        end
      },
      %{
        id: "prompt_low",
        hook: :before_prompt_build,
        priority: 10,
        handler: fn _event, _context ->
          %{"system_prompt" => "low", "prepend_context" => "[low]"}
        end
      },
      %{
        id: "persist_marker",
        hook: :before_message_persist,
        priority: 10,
        handler: fn _event, _context ->
          %{"payload" => %{"hook_marker" => "persisted"}}
        end
      }
    ]

    assert {:ok, %{processed_frames: 2, status: "succeeded", terminal_reason_class: "completed"}} =
             Executor.run_once(run_id, lease_owner: "worker-hooks", hooks: hooks)

    events = RunEvents.list_after(run_id, 0)

    assert Enum.map(events, & &1.event_type) == [
             "run.started",
             "run.hook_applied",
             "run.hook_applied",
             "run.hook_applied",
             "run.hook_applied",
             "run.delta",
             "run.hook_applied",
             "run.finished"
           ]

    before_prompt_applied =
      events
      |> Enum.filter(&(&1.event_type == "run.hook_applied"))
      |> Enum.filter(&(&1.payload["hook_name"] == "before_prompt_build"))

    assert Enum.map(before_prompt_applied, & &1.payload["hook_id"]) == [
             "prompt_high",
             "prompt_low"
           ]

    [delta_event] = Enum.filter(events, &(&1.event_type == "run.delta"))
    assert delta_event.payload["delta"] == "[high]\n\n[low]\n\nhello"
    assert delta_event.payload["hook_marker"] == "persisted"

    [finish_event] = Enum.filter(events, &(&1.event_type == "run.finished"))
    assert finish_event.payload["status"] == "succeeded"
    assert finish_event.payload["hook_marker"] == "persisted"
  end

  test "hook failures are bounded and observable without breaking run safety" do
    run_id = unique_run_id("exec_hook_fail")
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

    hooks = [
      %{
        id: "prompt_raise",
        hook: :before_prompt_build,
        priority: 20,
        handler: fn _event, _context -> raise "hook exploded" end
      },
      %{
        id: "persist_invalid",
        hook: :before_message_persist,
        priority: 10,
        handler: fn _event, _context -> "invalid_result" end
      }
    ]

    assert {:ok, %{processed_frames: 2, status: "succeeded", terminal_reason_class: "completed"}} =
             Executor.run_once(run_id, lease_owner: "worker-hooks-fail", hooks: hooks)

    events = RunEvents.list_after(run_id, 0)

    assert Enum.any?(
             events,
             &(&1.event_type == "run.hook_error" and &1.payload["hook_id"] == "prompt_raise")
           )

    assert Enum.any?(
             events,
             &(&1.event_type == "run.hook_error" and &1.payload["hook_id"] == "persist_invalid")
           )

    assert Enum.any?(events, &(&1.event_type == "run.delta"))

    assert Enum.any?(
             events,
             &(&1.event_type == "run.finished" and &1.payload["status"] == "succeeded")
           )
  end

  test "runs before_tool_call and after_tool_call hooks deterministically" do
    run_id = unique_run_id("exec_tool_hooks")
    insert_run(run_id)

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "frame_1",
               type: "tool_call",
               payload: %{"tool" => "search_web"}
             })

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "frame_2",
               type: "tool_result",
               payload: %{"ok" => true}
             })

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "frame_3",
               type: "complete",
               payload: %{}
             })

    hooks = [
      %{
        id: "before_tool",
        hook: :before_tool_call,
        priority: 10,
        handler: fn _event, _context -> %{"payload" => %{"text" => "tool call patched"}} end
      },
      %{
        id: "after_tool",
        hook: :after_tool_call,
        priority: 10,
        handler: fn _event, _context -> :ok end
      }
    ]

    assert {:ok, %{processed_frames: 3, status: "succeeded", terminal_reason_class: "completed"}} =
             Executor.run_once(run_id, lease_owner: "worker-tool-hooks", hooks: hooks)

    events = RunEvents.list_after(run_id, 0)

    assert Enum.any?(
             events,
             &(&1.event_type == "run.hook_applied" and &1.payload["hook_id"] == "before_tool")
           )

    assert Enum.any?(
             events,
             &(&1.event_type == "run.hook_applied" and &1.payload["hook_id"] == "after_tool")
           )

    assert Enum.any?(
             events,
             &(&1.event_type == "run.delta" and &1.payload["delta"] == "tool call patched")
           )
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
