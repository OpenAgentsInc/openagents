defmodule OpenAgentsRuntime.DS.Workflows.StructuredTasksTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.DS.Workflows.StructuredTasks

  test "run/3 executes llm_task.v1 with typed output and replayable workflow receipt" do
    input = %{
      "task" => %{"id" => "task_1", "objective" => "Choose next action"},
      "context" => %{"thread_id" => "thread_1"},
      "tools" => [%{"name" => "search.docs", "description" => "Search docs"}]
    }

    assert {:ok, result} =
             StructuredTasks.run("llm_task.v1", input,
               run_id: "wf_llm_1",
               strategy_id: "direct.v1",
               budget: %{"remaining_sats" => 5, "max_steps" => 4, "step_cost_sats" => 1}
             )

    assert result["workflow_id"] == "llm_task.v1"
    assert result["output"]["status"] == "completed"
    assert is_map(result["output"]["result"])
    assert result["receipt"]["step_count"] == 1
    assert result["receipt"]["strategy_ids"] == ["direct.v1"]
    assert String.length(result["receipt"]["replay_hash"]) == 64
    assert result["receipt"]["budget_after"]["remaining_sats"] == 4
  end

  test "run/3 executes timeline_map_reduce.v1 with bounded map fanout and trace linkage" do
    input = %{
      "query" => "find timeline anomalies",
      "items" => [
        %{"minute" => "00:00", "event" => "agent.start"},
        %{"minute" => "00:01", "event" => "tool.call"},
        %{"minute" => "00:02", "event" => "tool.result"}
      ]
    }

    assert {:ok, result} =
             StructuredTasks.run("timeline_map_reduce.v1", input,
               run_id: "wf_map_reduce_1",
               budget: %{"remaining_sats" => 10, "max_map_items" => 2, "step_cost_sats" => 1},
               map_strategy_id: "direct.v1",
               reduce_strategy_id: "rlm_lite.v1",
               max_iterations: 3
             )

    assert result["workflow_id"] == "timeline_map_reduce.v1"
    assert result["output"]["item_count"] == 2
    assert result["receipt"]["step_count"] == 3
    assert Enum.member?(result["receipt"]["strategy_ids"], "direct.v1")
    assert Enum.member?(result["receipt"]["strategy_ids"], "rlm_lite.v1")
    assert length(result["trace_links"]["trace_refs"]) >= 1
  end

  test "run/3 returns budget_exhausted deterministically when step budget is depleted" do
    handler_id = "parity-workflow-#{System.unique_integer([:positive])}"

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

    input = %{
      "task" => %{"id" => "task_1", "objective" => "budget test"},
      "context" => %{},
      "tools" => []
    }

    assert {:error, :budget_exhausted} =
             StructuredTasks.run("llm_task.v1", input,
               run_id: "wf_budget_1",
               budget: %{"remaining_sats" => 0}
             )

    assert_receive {:parity_failure, %{count: 1}, metadata}, 1_000
    assert metadata.class == "workflow"
    assert metadata.reason_class == "policy_denied.budget_exhausted"
    assert metadata.component == "ds.workflows"
  end

  test "run/3 returns step_failed when workflow step strategy is unsupported" do
    input = %{
      "query" => "diagnose",
      "items" => [%{"event" => "one"}]
    }

    assert {:error, {:step_failed, "reduce", {:unsupported_strategy, "bad.v9"}}} =
             StructuredTasks.run("timeline_map_reduce.v1", input,
               run_id: "wf_bad_strategy",
               budget: %{"remaining_sats" => 5},
               reduce_strategy_id: "bad.v9"
             )
  end

  test "run/3 enforces typed workflow input contract" do
    assert {:error, {:schema_violation, reason}} =
             StructuredTasks.run(
               "llm_task.v1",
               %{
                 "task" => %{"id" => "task_1"},
                 "context" => %{},
                 "tools" => []
               },
               run_id: "wf_schema"
             )

    assert String.contains?(reason, "input.task.objective")
  end
end
