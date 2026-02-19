defmodule OpenAgentsRuntime.Tools.Coding.Kernel do
  @moduledoc """
  Runtime coding kernel for GitHub-oriented coding operations.

  This mirrors OpenClaw's coding-oriented GitHub skill surface through a typed,
  receipt-visible runtime path instead of direct CLI invocation.
  """

  alias OpenAgentsRuntime.DS.PolicyEvaluator
  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.Security.Sanitizer
  alias OpenAgentsRuntime.Tools.Coding.Providers.GitHubAdapter
  alias OpenAgentsRuntime.Tools.Extensions.ManifestRegistry
  alias OpenAgentsRuntime.Tools.ProviderCircuitBreaker

  @required_request_fields ~w(integration_id operation)
  @write_operations MapSet.new(["add_issue_comment"])

  @type coding_outcome :: %{required(String.t()) => term()}

  @spec execute_operation(map(), map(), keyword()) ::
          {:ok, coding_outcome()}
          | {:error, {:invalid_manifest, [map()]}}
          | {:error, {:invalid_request, [String.t()]}}
  def execute_operation(manifest, request, opts \\ [])
      when is_map(manifest) and is_map(request) do
    with {:ok, manifest} <- ManifestRegistry.validate_for_activation(manifest),
         {:ok, request} <- validate_request(request, manifest),
         {:ok, policy_context} <- evaluate_request_policy(manifest, request, opts) do
      if policy_context["evaluation"]["decision"] == "denied" do
        {:ok, blocked_outcome(manifest, request, policy_context)}
      else
        execute_provider_operation(manifest, request, policy_context, opts)
      end
    else
      {:error, {:invalid_request, errors}} -> {:error, {:invalid_request, errors}}
      {:error, reason} -> {:error, reason}
    end
  end

  @doc """
  Replays coding decision path without invoking provider adapters.
  """
  @spec replay_decision(map(), map(), keyword()) ::
          {:ok, map()}
          | {:error, {:invalid_manifest, [map()]}}
          | {:error, {:invalid_request, [String.t()]}}
  def replay_decision(manifest, request, opts \\ []) when is_map(manifest) and is_map(request) do
    with {:ok, manifest} <- ManifestRegistry.validate_for_activation(manifest),
         {:ok, request} <- validate_request(request, manifest),
         {:ok, policy_context} <- evaluate_request_policy(manifest, request, opts) do
      policy_eval = policy_context["evaluation"]

      replay_hash =
        Receipts.stable_hash(%{
          "manifest_id" => manifest["integration_id"],
          "request" => Sanitizer.sanitize(request),
          "decision" => policy_eval["decision"],
          "reason_code" => policy_eval["reason_code"],
          "evaluation_hash" => policy_eval["evaluation_hash"]
        })

      {:ok,
       %{
         "decision" => policy_eval["decision"],
         "reason_code" => policy_eval["reason_code"],
         "evaluation_hash" => policy_eval["evaluation_hash"],
         "replay_hash" => replay_hash
       }}
    end
  end

  defp execute_provider_operation(manifest, request, policy_context, opts) do
    adapter = Keyword.get(opts, :adapter, GitHubAdapter)
    provider = normalize_provider(manifest["provider"])
    base_policy = policy_context["base_policy"] || %{}
    budget = policy_context["budget"] || %{}

    case call_provider(adapter, provider, request, manifest, opts) do
      {:ok, provider_result} ->
        finalize_provider_outcome(manifest, request, base_policy, budget, provider_result)

      {:error, failure_reason} ->
        denied_reason_code =
          case failure_reason do
            :provider_circuit_open ->
              "coding_failed.provider_circuit_open"

            {:provider_error, provider_error} ->
              normalize_reason_code(provider_error["reason_code"], "coding_failed.provider_error")

            _ ->
              "coding_failed.provider_error"
          end

        provider_failure_result = provider_failure_result(provider, failure_reason)

        failure_policy =
          evaluate_outcome_policy(base_policy, budget, "denied", denied_reason_code)

        {:ok,
         build_outcome(
           manifest,
           request,
           "failed",
           provider_failure_result,
           failure_policy
         )}
    end
  end

  defp finalize_provider_outcome(manifest, request, base_policy, budget, provider_result) do
    provider_result = normalize_map(provider_result)
    state = provider_result["state"] || "succeeded"
    success? = state == "succeeded"

    reason_code =
      provider_result["reason_code"] ||
        if(success?, do: "policy_allowed.default", else: "coding_failed.provider_error")

    reason_code =
      normalize_reason_code(
        reason_code,
        if(success?, do: "policy_allowed.default", else: "coding_failed.provider_error")
      )

    policy_eval =
      evaluate_outcome_policy(
        base_policy,
        budget,
        if(success?, do: "allowed", else: "denied"),
        reason_code
      )

    {:ok, build_outcome(manifest, request, state, provider_result, policy_eval)}
  end

  defp blocked_outcome(manifest, request, policy_context) do
    policy_eval = policy_context["evaluation"] || %{}
    build_outcome(manifest, request, "blocked", %{}, policy_eval)
  end

  defp build_outcome(manifest, request, state, provider_result, policy_eval) do
    integration_id = manifest["integration_id"] || manifest["extension_id"]
    provider = normalize_provider(manifest["provider"])
    operation = request["operation"]

    receipt_payload =
      %{
        "integration_id" => integration_id,
        "provider" => provider,
        "operation" => operation,
        "state" => state,
        "decision" => policy_eval["decision"],
        "reason_code" => policy_eval["reason_code"],
        "evaluation_hash" => policy_eval["evaluation_hash"],
        "reason_codes_version" => policy_eval["reason_codes_version"]
      }
      |> maybe_put("result_hash", Receipts.stable_hash(provider_result))

    %{
      "integration_id" => integration_id,
      "provider" => provider,
      "operation" => operation,
      "state" => state,
      "decision" => policy_eval["decision"],
      "reason_code" => policy_eval["reason_code"],
      "policy" => policy_eval,
      "provider_result" => Sanitizer.sanitize(provider_result),
      "receipt" =>
        receipt_payload
        |> Map.put(
          "receipt_id",
          "coding_" <> String.slice(Receipts.stable_hash(receipt_payload), 0, 24)
        )
        |> Map.put("replay_hash", Receipts.stable_hash(receipt_payload))
    }
  end

  defp call_provider(adapter, provider, request, manifest, opts) do
    breaker_opts =
      opts
      |> Keyword.get(:breaker_opts, [])
      |> Keyword.put_new(:failure_threshold, Keyword.get(opts, :provider_failure_threshold, 3))
      |> Keyword.put_new(:reset_timeout_ms, Keyword.get(opts, :provider_reset_timeout_ms, 10_000))
      |> Keyword.put_new(:time_provider, Keyword.get(opts, :provider_breaker_time_provider))

    case ProviderCircuitBreaker.call(
           provider,
           fn -> adapter.execute(request, manifest, opts) end,
           breaker_opts
         ) do
      {:error, :circuit_open} ->
        {:error, :provider_circuit_open}

      {:ok, result} when is_map(result) ->
        {:ok, Map.put(normalize_map(result), "provider", provider)}

      {:error, result} when is_map(result) ->
        {:error, {:provider_error, result |> normalize_map() |> Map.put("provider", provider)}}

      {:error, _reason} ->
        {:error, :provider_error}

      _other ->
        {:error, :provider_error}
    end
  end

  defp evaluate_request_policy(manifest, request, opts) do
    write_mode = get_in(manifest, ["policy", "write_operations_mode"]) || "enforce"
    write_operation? = MapSet.member?(@write_operations, request["operation"])

    base_policy =
      opts
      |> Keyword.get(:policy, %{})
      |> normalize_map()
      |> maybe_put(
        "authorization_id",
        normalize_optional_string(Keyword.get(opts, :authorization_id))
      )
      |> maybe_put(
        "authorization_mode",
        normalize_optional_string(Keyword.get(opts, :authorization_mode))
      )
      |> Map.put("policy_id", "coding.execute.v1")
      |> maybe_put("decision", pre_decision(write_mode, write_operation?, request, opts))
      |> maybe_put("reason_code", pre_reason_code(write_mode, write_operation?, request, opts))

    budget = normalize_map(Keyword.get(opts, :budget, %{}))
    evaluation = PolicyEvaluator.evaluate(base_policy, budget, %{})

    {:ok,
     %{
       "evaluation" => evaluation,
       "base_policy" => base_policy,
       "budget" => budget
     }}
  end

  defp pre_decision(write_mode, true, request, opts) when write_mode == "enforce" do
    if write_approved?(request, opts), do: nil, else: "denied"
  end

  defp pre_decision(_write_mode, _write_operation, _request, _opts), do: nil

  defp pre_reason_code(write_mode, true, request, opts) when write_mode == "enforce" do
    if write_approved?(request, opts), do: nil, else: "policy_denied.explicit_deny"
  end

  defp pre_reason_code(_write_mode, _write_operation, _request, _opts), do: nil

  defp write_approved?(request, opts) do
    request["write_approved"] == true || Keyword.get(opts, :write_approved) == true
  end

  defp validate_request(request, manifest) do
    request = normalize_map(request)

    missing_fields =
      @required_request_fields
      |> Enum.reject(&present_string?(request[&1]))

    errors =
      []
      |> maybe_add_error(
        missing_fields != [],
        "missing required fields: #{Enum.join(missing_fields, ", ")}"
      )
      |> validate_operation(request)
      |> validate_repository(request, manifest)
      |> validate_operation_payload(request)
      |> validate_manifest_capability(request, manifest)
      |> Enum.reverse()

    if errors == [], do: {:ok, request}, else: {:error, {:invalid_request, errors}}
  end

  defp validate_operation(errors, request) do
    operation = normalize_optional_string(request["operation"])

    if operation in ["get_issue", "get_pull_request", "add_issue_comment"] do
      errors
    else
      ["operation is not supported" | errors]
    end
  end

  defp validate_repository(errors, request, manifest) do
    repository =
      normalize_optional_string(
        request["repository"] || get_in(manifest, ["policy", "default_repository"])
      )

    if valid_repository?(repository) do
      errors
    else
      ["repository is required and must be in 'owner/repo' format" | errors]
    end
  end

  defp validate_operation_payload(errors, request) do
    operation = request["operation"]

    cond do
      operation in ["get_issue", "add_issue_comment"] and
          not positive_integer?(request["issue_number"]) ->
        ["issue_number must be a positive integer" | errors]

      operation == "get_pull_request" and not positive_integer?(request["pull_number"]) ->
        ["pull_number must be a positive integer" | errors]

      operation == "add_issue_comment" and not present_string?(request["body"]) ->
        ["body is required for add_issue_comment" | errors]

      true ->
        errors
    end
  end

  defp validate_manifest_capability(errors, request, manifest) do
    capabilities = MapSet.new(List.wrap(manifest["capabilities"]))

    if MapSet.member?(capabilities, request["operation"]) do
      errors
    else
      ["operation is not enabled by manifest capabilities" | errors]
    end
  end

  defp provider_failure_result(provider, failure_reason) do
    base = %{"provider" => provider, "fallback_used" => false}

    case failure_reason do
      {:provider_error, provider_error} ->
        provider_error
        |> normalize_map()
        |> Map.merge(base)
        |> maybe_put("failure_reason", provider_error["message"] || "provider_error")

      reason ->
        Map.put(base, "failure_reason", to_string(reason))
    end
  end

  defp normalize_reason_code(reason_code, fallback) when is_binary(reason_code) do
    normalized = String.trim(reason_code)

    cond do
      normalized == "" -> fallback
      normalized in OpenAgentsRuntime.DS.PolicyReasonCodes.all() -> normalized
      true -> fallback
    end
  end

  defp normalize_reason_code(_reason_code, fallback), do: fallback

  defp evaluate_outcome_policy(base_policy, budget, decision, reason_code) do
    base_policy
    |> Map.put("decision", decision)
    |> Map.put("reason_code", reason_code)
    |> PolicyEvaluator.evaluate(budget, %{})
  end

  defp normalize_provider(provider) when is_binary(provider), do: provider
  defp normalize_provider(provider), do: to_string(provider || "unknown")

  defp maybe_add_error(errors, false, _message), do: errors
  defp maybe_add_error(errors, true, message), do: [message | errors]

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)

  defp normalize_map(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp normalize_map(_), do: %{}

  defp normalize_optional_string(nil), do: nil

  defp normalize_optional_string(value) when is_binary(value) do
    trimmed = String.trim(value)
    if trimmed == "", do: nil, else: trimmed
  end

  defp normalize_optional_string(value) when is_atom(value),
    do: value |> Atom.to_string() |> normalize_optional_string()

  defp normalize_optional_string(value) when is_integer(value), do: Integer.to_string(value)

  defp normalize_optional_string(_), do: nil

  defp present_string?(nil), do: false
  defp present_string?(value) when is_binary(value), do: String.trim(value) != ""

  defp present_string?(value) when is_atom(value),
    do: value |> Atom.to_string() |> present_string?()

  defp present_string?(value) when is_integer(value), do: value > 0
  defp present_string?(_), do: false

  defp positive_integer?(value) when is_integer(value), do: value > 0

  defp positive_integer?(value) when is_binary(value) do
    case Integer.parse(String.trim(value)) do
      {parsed, ""} -> parsed > 0
      _ -> false
    end
  end

  defp positive_integer?(_), do: false

  defp valid_repository?(repository) when is_binary(repository) do
    String.match?(repository, ~r/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/)
  end

  defp valid_repository?(_), do: false
end
