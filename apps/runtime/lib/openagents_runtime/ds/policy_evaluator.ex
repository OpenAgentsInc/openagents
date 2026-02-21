defmodule OpenAgentsRuntime.DS.PolicyEvaluator do
  @moduledoc """
  Deterministic policy evaluation and replay helper.

  The evaluator emits a stable decision + reason code + hash so policy outcomes
  can be replayed from event/auth context.
  """

  alias OpenAgentsRuntime.DS.PolicyReasonCodes
  alias OpenAgentsRuntime.DS.Receipts

  @allowed_authorization_modes MapSet.new([
                                 "delegated_budget",
                                 "delegated_budget_with_threshold",
                                 "interactive",
                                 "deny",
                                 "system"
                               ])

  @spec evaluate(map(), map(), map()) :: map()
  def evaluate(policy, budget, context \\ %{})
      when is_map(policy) and is_map(budget) and is_map(context) do
    policy = stringify_keys(policy)
    budget = stringify_keys(budget)
    context = stringify_keys(context)

    {decision, reason_code} = resolve_decision(policy, budget, context)

    %{
      "decision" => decision,
      "reason_code" => reason_code,
      "reason_codes_version" => PolicyReasonCodes.version(),
      "evaluation_hash" =>
        Receipts.stable_hash(%{
          "decision" => decision,
          "reason_code" => reason_code,
          "policy" =>
            Map.take(policy, [
              "authorization_id",
              "authorization_mode",
              "decision",
              "reason_code",
              "policy_id"
            ]),
          "budget" => Map.take(budget, ["spent_sats", "reserved_sats", "remaining_sats"]),
          "context" =>
            Map.take(context, [
              "loop_detected_reason",
              "ssrf_block_reason",
              "manifest_validation_reason"
            ])
        })
    }
  end

  @doc """
  Re-evaluates policy from an event-log projection and auth context.
  """
  @spec replay(map(), map(), map()) :: map()
  def replay(event_log, authorization_context, budget \\ %{})
      when is_map(event_log) and is_map(authorization_context) and is_map(budget) do
    authorization_context = stringify_keys(authorization_context)
    event_log = stringify_keys(event_log)

    policy =
      %{}
      |> maybe_put("authorization_id", authorization_context["authorization_id"])
      |> maybe_put("authorization_mode", authorization_context["authorization_mode"])
      |> maybe_put("decision", authorization_context["decision"])
      |> maybe_put("reason_code", authorization_context["reason_code"])

    context =
      %{}
      |> maybe_put("loop_detected_reason", event_log["loop_detected_reason"])
      |> maybe_put("ssrf_block_reason", event_log["ssrf_block_reason"])
      |> maybe_put("manifest_validation_reason", event_log["manifest_validation_reason"])

    evaluate(policy, budget, context)
  end

  defp resolve_decision(policy, budget, context) do
    cond do
      manifest_reason =
          normalized_reason_code(context["manifest_validation_reason"], "manifest_validation.") ->
        {"denied", manifest_reason}

      loop_reason = normalized_reason_code(context["loop_detected_reason"], "loop_detected.") ->
        {"denied", loop_reason}

      ssrf_reason = normalized_reason_code(context["ssrf_block_reason"], "ssrf_block.") ->
        {"denied", ssrf_reason}

      policy["decision"] == "denied" and PolicyReasonCodes.valid?(policy["reason_code"]) ->
        {"denied", policy["reason_code"]}

      policy["decision"] == "denied" ->
        {"denied", "policy_denied.explicit_deny"}

      missing_authorization?(policy) ->
        {"denied", "policy_denied.authorization_missing"}

      invalid_authorization_mode?(policy["authorization_mode"]) ->
        {"denied", "policy_denied.invalid_authorization_mode"}

      budget_exhausted?(budget) ->
        {"denied", "policy_denied.budget_exhausted"}

      true ->
        {"allowed", "policy_allowed.default"}
    end
  end

  defp missing_authorization?(policy) do
    policy["authorization_id"]
    |> case do
      value when is_binary(value) -> String.trim(value) == ""
      nil -> true
      _ -> false
    end
  end

  defp invalid_authorization_mode?(nil), do: false

  defp invalid_authorization_mode?(authorization_mode) when is_binary(authorization_mode) do
    not MapSet.member?(@allowed_authorization_modes, String.trim(authorization_mode))
  end

  defp invalid_authorization_mode?(_), do: true

  defp budget_exhausted?(budget) do
    remaining = normalize_integer(budget["remaining_sats"])
    is_integer(remaining) and remaining <= 0
  end

  defp normalize_integer(value) when is_integer(value), do: value

  defp normalize_integer(value) when is_float(value), do: trunc(value)

  defp normalize_integer(value) when is_binary(value) do
    case Integer.parse(String.trim(value)) do
      {parsed, _rest} -> parsed
      :error -> nil
    end
  end

  defp normalize_integer(_), do: nil

  defp normalized_reason_code(nil, _prefix), do: nil

  defp normalized_reason_code(reason_code, expected_prefix) when is_binary(reason_code) do
    normalized = String.trim(reason_code)

    if normalized != "" and String.starts_with?(normalized, expected_prefix) and
         PolicyReasonCodes.valid?(normalized) do
      normalized
    else
      fallback_reason(expected_prefix)
    end
  end

  defp normalized_reason_code(_reason_code, expected_prefix), do: fallback_reason(expected_prefix)

  defp fallback_reason("manifest_validation."), do: "manifest_validation.invalid_schema"
  defp fallback_reason("loop_detected."), do: "loop_detected.no_progress"
  defp fallback_reason("ssrf_block."), do: "ssrf_block.private_address"
  defp fallback_reason(_), do: nil

  defp stringify_keys(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)
end
