defmodule OpenAgentsRuntime.Hooks.RunnerTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Hooks.Runner

  test "normalizes registrations and sorts deterministically by priority then id" do
    registry =
      Runner.normalize_registry([
        %{
          id: "low",
          hook: "before_prompt_build",
          priority: 1,
          handler: fn _event, _context -> %{"prepend_context" => "low"} end
        },
        %{
          id: "high",
          hook: :before_prompt_build,
          priority: 10,
          handler: fn _event, _context -> %{"prepend_context" => "high"} end
        },
        %{
          id: "a",
          hook: :before_prompt_build,
          priority: 10,
          handler: fn _event, _context -> %{"prepend_context" => "same-priority-a"} end
        },
        %{
          id: "invalid-hook",
          hook: "not_real",
          priority: 10,
          handler: fn _event, _context -> %{} end
        },
        %{
          id: "invalid-handler",
          hook: :before_prompt_build,
          priority: 10,
          handler: :not_a_function
        }
      ])

    assert Enum.map(registry, & &1.id) == ["a", "high", "low"]
    assert Enum.map(registry, & &1.priority) == [10, 10, 1]
  end

  test "modifying hook merge keeps higher priority override and preserves deterministic prepend order" do
    registry =
      Runner.normalize_registry([
        %{
          id: "high",
          hook: :before_prompt_build,
          priority: 20,
          handler: fn _event, _context ->
            %{
              "system_prompt" => "high prompt",
              "prepend_context" => "high context"
            }
          end
        },
        %{
          id: "low",
          hook: :before_prompt_build,
          priority: 10,
          handler: fn _event, _context ->
            %{
              "system_prompt" => "low prompt",
              "prepend_context" => "low context"
            }
          end
        }
      ])

    result =
      Runner.run_modifying_hook(
        registry,
        :before_prompt_build,
        %{"frame_type" => "user_message"},
        %{},
        &Runner.merge_before_prompt_build/2
      )

    assert result.result["system_prompt"] == "high prompt"
    assert result.result["prepend_context"] == "high context\n\nlow context"

    assert Enum.map(result.events, fn {event_type, payload} ->
             {event_type, payload["hook_id"]}
           end) == [
             {"run.hook_applied", "high"},
             {"run.hook_applied", "low"}
           ]
  end

  test "before_message_persist merge keeps higher priority event_type and payload keys stable" do
    registry =
      Runner.normalize_registry([
        %{
          id: "high",
          hook: :before_message_persist,
          priority: 20,
          handler: fn _event, _context ->
            %{
              "event_type" => "run.delta.high",
              "payload" => %{"priority" => "high", "overlap" => "high"}
            }
          end
        },
        %{
          id: "low",
          hook: :before_message_persist,
          priority: 10,
          handler: fn _event, _context ->
            %{
              "event_type" => "run.delta.low",
              "payload" => %{"secondary" => true, "overlap" => "low"}
            }
          end
        }
      ])

    result =
      Runner.run_modifying_hook(
        registry,
        :before_message_persist,
        %{"event_type" => "run.delta", "payload" => %{}},
        %{},
        &Runner.merge_before_message_persist/2
      )

    assert result.result["event_type"] == "run.delta.high"
    assert result.result["payload"]["priority"] == "high"
    assert result.result["payload"]["secondary"]
    assert result.result["payload"]["overlap"] == "high"
  end

  test "hook errors are bounded and observable for modifying and void phases" do
    registry =
      Runner.normalize_registry([
        %{
          id: "raise-modifying",
          hook: :before_prompt_build,
          priority: 10,
          handler: fn _event, _context -> raise "boom" end
        },
        %{
          id: "invalid-modifying",
          hook: :before_prompt_build,
          priority: 9,
          handler: fn _event, _context -> "invalid" end
        },
        %{
          id: "raise-void",
          hook: :after_tool_call,
          priority: 10,
          handler: fn _event, _context -> raise "boom" end
        }
      ])

    modifying_result =
      Runner.run_modifying_hook(
        registry,
        :before_prompt_build,
        %{"frame_type" => "user_message"},
        %{},
        &Runner.merge_before_prompt_build/2
      )

    assert modifying_result.result == nil

    assert Enum.map(modifying_result.events, fn {event_type, payload} ->
             {event_type, payload["hook_id"], payload["reason"]}
           end) == [
             {"run.hook_error", "raise-modifying", "boom"},
             {"run.hook_error", "invalid-modifying", "invalid_hook_result"}
           ]

    void_result = Runner.run_void_hook(registry, :after_tool_call, %{}, %{})

    assert Enum.map(void_result, fn {event_type, payload} ->
             {event_type, payload["hook_id"], payload["reason"]}
           end) == [
             {"run.hook_error", "raise-void", "boom"}
           ]
  end
end
