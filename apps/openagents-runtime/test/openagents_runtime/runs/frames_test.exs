defmodule OpenAgentsRuntime.Runs.FramesTest do
  use OpenAgentsRuntime.DataCase, async: true

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Frames
  alias OpenAgentsRuntime.Runs.Run

  setup do
    Repo.insert!(%Run{
      run_id: "run_frames_1",
      thread_id: "thread_frames_1",
      status: "running",
      owner_user_id: 44,
      latest_seq: 0
    })

    :ok
  end

  test "append_frame returns accepted on first write" do
    assert {:ok, %{idempotent_replay: false, frame: frame}} =
             Frames.append_frame("run_frames_1", %{
               frame_id: "frame_1",
               frame_type: "user_message",
               payload: %{"text" => "hello"}
             })

    assert frame.frame_id == "frame_1"
  end

  test "append_frame returns idempotent replay on duplicate payload" do
    assert {:ok, %{idempotent_replay: false}} =
             Frames.append_frame("run_frames_1", %{
               frame_id: "frame_1",
               frame_type: "user_message",
               payload: %{"text" => "same"}
             })

    assert {:ok, %{idempotent_replay: true, frame: frame}} =
             Frames.append_frame("run_frames_1", %{
               frame_id: "frame_1",
               frame_type: "user_message",
               payload: %{"text" => "same"}
             })

    assert frame.frame_id == "frame_1"
  end

  test "append_frame returns conflict on duplicate frame_id with changed payload" do
    assert {:ok, %{idempotent_replay: false}} =
             Frames.append_frame("run_frames_1", %{
               frame_id: "frame_conflict",
               frame_type: "user_message",
               payload: %{"text" => "v1"}
             })

    assert {:error, :idempotency_conflict} =
             Frames.append_frame("run_frames_1", %{
               frame_id: "frame_conflict",
               frame_type: "user_message",
               payload: %{"text" => "v2"}
             })
  end

  test "append_frame returns run_not_found when run is missing" do
    assert {:error, :run_not_found} =
             Frames.append_frame("missing_run", %{
               frame_id: "frame_missing",
               frame_type: "user_message",
               payload: %{}
             })
  end
end
