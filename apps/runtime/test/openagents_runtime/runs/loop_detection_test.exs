defmodule OpenAgentsRuntime.Runs.LoopDetectionTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Frames
  alias OpenAgentsRuntime.Runs.LoopDetection
  alias OpenAgentsRuntime.Runs.Run

  test "detect/3 reports generic_repeat for repeated frame signatures" do
    run_id = unique_run_id("loop_generic")
    insert_run(run_id)

    append_text_frame(run_id, "frame_1", "same")
    append_text_frame(run_id, "frame_2", "same")
    append_text_frame(run_id, "frame_3", "same")

    frame = Frames.get_frame(run_id, "frame_3")

    assert {:ok, detection} =
             LoopDetection.detect(run_id, frame,
               warning_threshold: 3,
               critical_threshold: 4,
               global_circuit_breaker_threshold: 10
             )

    assert detection.detector == :generic_repeat
    assert detection.reason_code == "loop_detected.no_progress"
    assert detection.count == 3
    assert detection.level == :warning
  end

  test "detect/3 reports known_poll_no_progress for repeated process poll frames" do
    run_id = unique_run_id("loop_poll")
    insert_run(run_id)

    append_poll_frame(run_id, "poll_1")
    append_poll_frame(run_id, "poll_2")
    append_poll_frame(run_id, "poll_3")

    frame = Frames.get_frame(run_id, "poll_3")

    assert {:ok, detection} =
             LoopDetection.detect(run_id, frame,
               warning_threshold: 3,
               critical_threshold: 4,
               global_circuit_breaker_threshold: 10
             )

    assert detection.detector == :known_poll_no_progress
    assert detection.reason_code == "loop_detected.no_progress"
    assert detection.count == 3
  end

  test "detect/3 reports ping_pong for alternating frame signatures" do
    run_id = unique_run_id("loop_ping_pong")
    insert_run(run_id)

    append_text_frame(run_id, "frame_a1", "A")
    append_text_frame(run_id, "frame_b1", "B")
    append_text_frame(run_id, "frame_a2", "A")
    append_text_frame(run_id, "frame_b2", "B")

    frame = Frames.get_frame(run_id, "frame_b2")

    assert {:ok, detection} =
             LoopDetection.detect(run_id, frame,
               warning_threshold: 4,
               critical_threshold: 6,
               global_circuit_breaker_threshold: 12
             )

    assert detection.detector == :ping_pong
    assert detection.reason_code == "loop_detected.no_progress"
    assert detection.count == 4
  end

  test "detect/3 reports global circuit breaker when loop-candidate count exceeds threshold" do
    run_id = unique_run_id("loop_global")
    insert_run(run_id)

    append_poll_frame(run_id, "poll_breaker_1")
    append_poll_frame(run_id, "poll_breaker_2")
    append_poll_frame(run_id, "poll_breaker_3")
    append_poll_frame(run_id, "poll_breaker_4")

    frame = Frames.get_frame(run_id, "poll_breaker_4")

    assert {:ok, detection} =
             LoopDetection.detect(run_id, frame,
               warning_threshold: 2,
               critical_threshold: 3,
               global_circuit_breaker_threshold: 4
             )

    assert detection.detector == :global_circuit_breaker
    assert detection.reason_code == "loop_detected.no_progress"
    assert detection.level == :critical
    assert detection.count == 4
  end

  defp append_text_frame(run_id, frame_id, text) do
    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: frame_id,
               type: "user_message",
               payload: %{"text" => text}
             })
  end

  defp append_poll_frame(run_id, frame_id) do
    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: frame_id,
               type: "tool_result",
               payload: %{
                 "tool_name" => "process",
                 "action" => "poll",
                 "status" => "running",
                 "text" => "no progress"
               }
             })
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
