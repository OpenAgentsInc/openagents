defmodule OpenAgentsRuntime.Spend.Policy do
  @moduledoc """
  Durable policy decision events for spend-enforced tool execution.
  """

  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Spend.Authorizations
  alias OpenAgentsRuntime.Spend.SpendAuthorization

  @spec emit_denial(String.t(), String.t(), map()) :: {:ok, map()} | {:error, term()}
  def emit_denial(run_id, tool_call_id, attrs)
      when is_binary(run_id) and is_binary(tool_call_id) and is_map(attrs) do
    authorization_id = attrs[:authorization_id] || attrs["authorization_id"]

    authorization_mode =
      attrs[:authorization_mode] || attrs["authorization_mode"] || "delegated_budget"

    reason_code = attrs[:reason_code] || attrs["reason_code"] || "policy_denied.explicit_deny"

    budget =
      attrs[:budget] || attrs["budget"] ||
        budget_snapshot(authorization_id)

    decision_payload = %{
      "decision" => "denied",
      "reason_code" => reason_code,
      "tool_call_id" => tool_call_id,
      "authorization_id" => authorization_id,
      "authorization_mode" => authorization_mode,
      "budget" => budget,
      "policy_decision_id" =>
        Receipts.stable_hash(%{
          "run_id" => run_id,
          "tool_call_id" => tool_call_id,
          "reason_code" => reason_code,
          "authorization_id" => authorization_id,
          "budget" => budget
        })
    }

    with {:ok, _decision_event} <-
           RunEvents.append_event(run_id, "policy.decision", decision_payload),
         {:ok, _event_or_noop} <- maybe_append_budget_exhausted(run_id, decision_payload) do
      {:ok, decision_payload}
    end
  end

  def emit_denial(_run_id, _tool_call_id, _attrs), do: {:error, :invalid_input}

  defp maybe_append_budget_exhausted(
         run_id,
         %{"reason_code" => "policy_denied.budget_exhausted"} = payload
       ) do
    RunEvents.append_event(run_id, "policy.budget_exhausted", payload)
  end

  defp maybe_append_budget_exhausted(_run_id, _payload), do: {:ok, :noop}

  defp budget_snapshot(nil), do: %{}

  defp budget_snapshot(authorization_id) when is_binary(authorization_id) do
    case Repo.get(SpendAuthorization, authorization_id) do
      %SpendAuthorization{} = authorization -> Authorizations.budget_snapshot(authorization)
      nil -> %{}
    end
  end

  defp budget_snapshot(_), do: %{}
end
