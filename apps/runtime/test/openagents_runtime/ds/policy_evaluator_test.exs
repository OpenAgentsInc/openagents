defmodule OpenAgentsRuntime.DS.PolicyEvaluatorTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.DS.PolicyEvaluator
  alias OpenAgentsRuntime.DS.PolicyReasonCodes

  test "evaluate/3 emits stable reason codes and deterministic evaluation hash" do
    policy = %{
      authorization_id: "auth_123",
      authorization_mode: "delegated_budget",
      policy_id: "policy_default"
    }

    budget = %{spent_sats: 10, reserved_sats: 2, remaining_sats: 90}

    context = %{
      loop_detected_reason: "loop_detected.no_progress"
    }

    evaluation_a = PolicyEvaluator.evaluate(policy, budget, context)
    evaluation_b = PolicyEvaluator.evaluate(policy, budget, context)

    assert evaluation_a == evaluation_b
    assert evaluation_a["decision"] == "denied"
    assert evaluation_a["reason_code"] == "loop_detected.no_progress"
    assert evaluation_a["reason_codes_version"] == PolicyReasonCodes.version()
    assert String.length(evaluation_a["evaluation_hash"]) == 64
    assert PolicyReasonCodes.valid?(evaluation_a["reason_code"])
  end

  test "replay/3 reproduces deny decisions from event log and auth context" do
    event_log = %{
      loop_detected_reason: "loop_detected.no_progress"
    }

    authorization_context = %{
      authorization_id: "auth_999",
      authorization_mode: "delegated_budget"
    }

    budget = %{remaining_sats: 50}

    replay_a = PolicyEvaluator.replay(event_log, authorization_context, budget)
    replay_b = PolicyEvaluator.replay(event_log, authorization_context, budget)

    assert replay_a == replay_b
    assert replay_a["decision"] == "denied"
    assert replay_a["reason_code"] == "loop_detected.no_progress"
    assert replay_a["reason_codes_version"] == PolicyReasonCodes.version()
    assert String.length(replay_a["evaluation_hash"]) == 64
  end

  test "evaluate/3 denies exhausted budgets deterministically" do
    evaluation =
      PolicyEvaluator.evaluate(
        %{authorization_id: "auth_budget", authorization_mode: "delegated_budget"},
        %{remaining_sats: 0},
        %{}
      )

    assert evaluation["decision"] == "denied"
    assert evaluation["reason_code"] == "policy_denied.budget_exhausted"
    assert PolicyReasonCodes.valid?(evaluation["reason_code"])
  end
end
