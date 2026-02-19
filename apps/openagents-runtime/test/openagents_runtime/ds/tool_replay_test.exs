defmodule OpenAgentsRuntime.DS.ToolReplayTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.DS.ToolReplay
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Tools.ToolTasks

  test "build keeps replay bounded and deterministic under high tool volume" do
    run_id = unique_run_id("replay_bounded")
    insert_run(run_id)

    for index <- 1..25 do
      create_succeeded_task(run_id, "call_#{index}", %{
        "chunk" => String.duplicate("x", 180),
        "index" => index
      })
    end

    replay_a =
      ToolReplay.build(run_id, max_items: 10, max_item_chars: 70, max_total_chars: 750)

    replay_b =
      ToolReplay.build(run_id, max_items: 10, max_item_chars: 70, max_total_chars: 750)

    assert replay_a == replay_b
    assert replay_a["window"]["included_items"] <= 10
    assert replay_a["window"]["truncated_items"] > 0
    assert String.length(replay_a["summary"]) <= 750
  end

  test "build redacts sensitive material from replay output" do
    run_id = unique_run_id("replay_redaction")
    insert_run(run_id)

    create_succeeded_task(
      run_id,
      "call_sensitive",
      %{
        "api_key" => "sk-live-12345",
        "authorization" => "Bearer abc.def.ghi",
        "email" => "alice@example.com",
        "nested" => %{"password" => "supersecret", "safe" => "ok"}
      },
      %{
        "token" => "secret-token",
        "query" => "public"
      }
    )

    replay = ToolReplay.build(run_id, max_items: 5, max_item_chars: 200, max_total_chars: 1_200)
    encoded = Jason.encode!(replay)

    refute String.contains?(encoded, "sk-live-12345")
    refute String.contains?(encoded, "supersecret")
    refute String.contains?(encoded, "alice@example.com")
    assert String.contains?(encoded, "[REDACTED]")
  end

  defp create_succeeded_task(run_id, tool_call_id, output, input \\ %{}) do
    assert {:ok, %{task: queued}} =
             ToolTasks.enqueue(%{
               run_id: run_id,
               tool_call_id: tool_call_id,
               tool_name: "test.tool",
               input: input
             })

    assert {:ok, running} = ToolTasks.transition(queued, "running")

    assert {:ok, streaming} =
             ToolTasks.transition(running, "streaming", %{progress: %{"phase" => "work"}})

    assert {:ok, _succeeded} = ToolTasks.transition(streaming, "succeeded", %{output: output})
  end

  defp insert_run(run_id) do
    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: "thread_#{run_id}",
      status: "running",
      owner_user_id: 808,
      latest_seq: 0
    })
  end

  defp unique_run_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end
end
