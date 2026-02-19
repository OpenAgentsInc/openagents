defmodule OpenAgentsRuntime.Tools.ToolTasksTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Tools.ToolTasks

  test "enqueue persists queued tool task and enforces idempotent tool_call_id" do
    run_id = unique_run_id("tool_task_enqueue")
    insert_run(run_id)

    assert {:ok, %{task: task, idempotent_replay: false}} =
             ToolTasks.enqueue(%{
               run_id: run_id,
               tool_call_id: "tool_1",
               tool_name: "web.search",
               input: %{"q" => "elixir"},
               metadata: %{"attempt" => 1}
             })

    assert task.state == "queued"
    assert task.tool_call_id == "tool_1"
    assert %DateTime{} = task.queued_at

    assert {:ok, %{task: replay, idempotent_replay: true}} =
             ToolTasks.enqueue(%{
               run_id: run_id,
               tool_call_id: "tool_1",
               tool_name: "web.search",
               input: %{"q" => "changed"}
             })

    assert replay.id == task.id
  end

  test "supports legal queued -> running -> streaming -> succeeded transitions" do
    run_id = unique_run_id("tool_task_success")
    insert_run(run_id)

    assert {:ok, %{task: queued, idempotent_replay: false}} =
             ToolTasks.enqueue(%{
               run_id: run_id,
               tool_call_id: "tool_success",
               tool_name: "wallet.lookup",
               input: %{"address" => "abc"}
             })

    assert {:ok, running} = ToolTasks.transition(queued, "running")
    assert running.state == "running"
    assert %DateTime{} = running.running_at

    assert {:ok, streaming} =
             ToolTasks.transition(running, "streaming", %{progress: %{"phase" => "half"}})

    assert streaming.state == "streaming"
    assert %DateTime{} = streaming.streaming_at
    assert streaming.output == %{"phase" => "half"}

    assert {:ok, succeeded} =
             ToolTasks.transition(streaming, "succeeded", %{
               output: %{"result" => "ok"},
               metadata: %{"duration_ms" => 120}
             })

    assert succeeded.state == "succeeded"
    assert %DateTime{} = succeeded.succeeded_at
    assert succeeded.output == %{"result" => "ok"}
    assert succeeded.metadata["duration_ms"] == 120
  end

  test "rejects invalid transitions and persists failure classification fields" do
    run_id = unique_run_id("tool_task_failure")
    insert_run(run_id)

    assert {:ok, %{task: queued}} =
             ToolTasks.enqueue(%{
               run_id: run_id,
               tool_call_id: "tool_fail",
               tool_name: "http.fetch",
               input: %{"url" => "https://example.com"}
             })

    assert {:error, :invalid_transition} = ToolTasks.transition(queued, "succeeded")

    assert {:ok, running} = ToolTasks.transition(queued, "running")

    assert {:ok, failed} =
             ToolTasks.transition(running, "failed", %{
               error_class: "timeout",
               error_message: "provider timeout",
               output: %{"attempted" => true}
             })

    assert failed.state == "failed"
    assert failed.error_class == "timeout"
    assert failed.error_message == "provider timeout"
    assert failed.output == %{"attempted" => true}
    assert %DateTime{} = failed.failed_at

    assert {:error, :invalid_transition} = ToolTasks.transition(failed, "running")
  end

  defp insert_run(run_id) do
    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: "thread_#{run_id}",
      status: "running",
      owner_user_id: 99,
      latest_seq: 0
    })
  end

  defp unique_run_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end
end
