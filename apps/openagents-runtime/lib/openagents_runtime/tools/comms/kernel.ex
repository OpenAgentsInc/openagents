defmodule OpenAgentsRuntime.Tools.Comms.Kernel do
  @moduledoc """
  Runtime comms kernel for send-side policy/consent/suppression enforcement.
  """

  alias OpenAgentsRuntime.DS.PolicyEvaluator
  alias OpenAgentsRuntime.DS.PolicyReasonCodes
  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.Contracts.Layer0TypeAdapters
  alias OpenAgentsRuntime.Security.Sanitizer
  alias OpenAgentsRuntime.Tools.Comms.NoopAdapter
  alias OpenAgentsRuntime.Tools.ProviderCircuitBreaker
  alias OpenAgentsRuntime.Tools.Extensions.CommsManifestValidator

  @required_request_fields ~w(integration_id recipient template_id variables)
  @send_success_states MapSet.new(["queued", "sent"])

  @type send_outcome :: %{
          required(String.t()) => term()
        }

  @spec execute_send(map(), map(), keyword()) ::
          {:ok, send_outcome()}
          | {:error, {:invalid_manifest, [String.t()]}}
          | {:error, {:invalid_request, [String.t()]}}
  def execute_send(manifest, request, opts \\ []) when is_map(manifest) and is_map(request) do
    with {:ok, manifest} <- CommsManifestValidator.validate(manifest),
         {:ok, request} <- validate_request(request, manifest),
         {:ok, _intent} <- validate_comms_intent(manifest, request, opts) do
      adapter = Keyword.get(opts, :adapter, NoopAdapter)
      policy_context = normalize_map(Keyword.get(opts, :policy_context, %{}))
      budget = normalize_map(Keyword.get(opts, :budget, %{}))
      base_policy = build_base_policy(opts)
      authorization_id = base_policy["authorization_id"]

      case evaluate_policy_gate(manifest, request, base_policy, budget, policy_context, opts) do
        {:blocked, policy_eval} ->
          {:ok,
           build_outcome(
             manifest,
             request,
             policy_eval,
             "blocked",
             nil,
             authorization_id,
             %{}
           )}

        {:allowed, policy_eval} ->
          execute_provider_send(
            adapter,
            manifest,
            request,
            policy_eval,
            authorization_id,
            opts
          )
      end
    else
      {:error, {:invalid_comms_intent, errors}} ->
        {:error, {:invalid_request, errors}}

      {:error, reason} ->
        {:error, reason}
    end
  end

  @doc """
  Replays comms policy/guard decisions without invoking provider adapters.
  """
  @spec replay_decision(map(), map(), keyword()) ::
          {:ok, map()}
          | {:error, {:invalid_manifest, [String.t()]}}
          | {:error, {:invalid_request, [String.t()]}}
  def replay_decision(manifest, request, opts \\ []) when is_map(manifest) and is_map(request) do
    with {:ok, manifest} <- CommsManifestValidator.validate(manifest),
         {:ok, request} <- validate_request(request, manifest) do
      policy_context = normalize_map(Keyword.get(opts, :policy_context, %{}))
      budget = normalize_map(Keyword.get(opts, :budget, %{}))
      base_policy = build_base_policy(opts)

      {decision, policy_eval} =
        case evaluate_policy_gate(manifest, request, base_policy, budget, policy_context, opts) do
          {:blocked, eval} -> {"denied", eval}
          {:allowed, eval} -> {"allowed", eval}
        end

      replay_hash =
        Receipts.stable_hash(%{
          "manifest_id" => manifest["integration_id"],
          "request" => Sanitizer.sanitize(request),
          "decision" => decision,
          "reason_code" => policy_eval["reason_code"],
          "evaluation_hash" => policy_eval["evaluation_hash"]
        })

      {:ok,
       %{
         "decision" => decision,
         "reason_code" => policy_eval["reason_code"],
         "evaluation_hash" => policy_eval["evaluation_hash"],
         "replay_hash" => replay_hash
       }}
    end
  end

  defp execute_provider_send(adapter, manifest, request, policy_eval, authorization_id, opts) do
    provider = normalize_provider(manifest["provider"])

    case call_provider(adapter, provider, request, manifest, opts) do
      {:ok, provider_result} when is_map(provider_result) ->
        finalize_provider_outcome(
          manifest,
          request,
          policy_eval,
          authorization_id,
          provider_result
        )

      {:error, failure_reason} ->
        case maybe_execute_fallback(manifest, request, opts, failure_reason) do
          {:ok, fallback_result} ->
            finalize_provider_outcome(
              manifest,
              request,
              policy_eval,
              authorization_id,
              fallback_result
            )

          {:error, fallback_reason} ->
            denied_reason_code =
              case {failure_reason, fallback_reason} do
                {_, :fallback_exhausted} -> "comms_failed.fallback_exhausted"
                {:provider_circuit_open, _} -> "comms_failed.provider_circuit_open"
                _ -> "comms_failed.provider_error"
              end

            policy_eval =
              policy_eval
              |> Map.put("decision", "denied")
              |> Map.put("reason_code", denied_reason_code)

            {:ok,
             build_outcome(
               manifest,
               request,
               policy_eval,
               "failed",
               nil,
               authorization_id,
               %{
                 "provider" => provider,
                 "fallback_used" => false,
                 "failure_reason" => to_string(failure_reason)
               }
             )}
        end
    end
  end

  defp finalize_provider_outcome(
         manifest,
         request,
         policy_eval,
         authorization_id,
         provider_result
       ) do
    provider_result = normalize_map(provider_result)
    state = provider_result["state"] || "sent"
    message_id = provider_result["message_id"]

    fallback =
      if provider_result["fallback_used"] == true do
        %{
          "used" => true,
          "provider" => provider_result["provider"],
          "primary_provider" => provider_result["primary_provider"],
          "reason" => provider_result["fallback_reason"]
        }
      else
        nil
      end

    reason_code =
      provider_result["reason_code"] ||
        if(MapSet.member?(@send_success_states, state),
          do: "policy_allowed.default",
          else: "comms_failed.provider_error"
        )

    reason_code =
      normalize_reason_code(
        reason_code,
        if(MapSet.member?(@send_success_states, state),
          do: "policy_allowed.default",
          else: "comms_failed.provider_error"
        )
      )

    decision = if(MapSet.member?(@send_success_states, state), do: "allowed", else: "denied")

    policy_eval =
      policy_eval
      |> Map.put("decision", decision)
      |> Map.put("reason_code", reason_code)

    {:ok,
     build_outcome(
       manifest,
       request,
       policy_eval,
       state,
       message_id,
       authorization_id,
       provider_result,
       fallback: fallback
     )}
  end

  defp call_provider(adapter, provider, request, manifest, opts) do
    breaker_opts = breaker_opts(opts)

    case ProviderCircuitBreaker.call(
           provider,
           fn -> adapter.send(request, manifest, opts) end,
           breaker_opts
         ) do
      {:error, :circuit_open} ->
        {:error, :provider_circuit_open}

      {:ok, result} when is_map(result) ->
        {:ok, Map.put(normalize_map(result), "provider", provider)}

      {:error, _reason} ->
        {:error, :provider_error}

      _other ->
        {:error, :provider_error}
    end
  end

  defp maybe_execute_fallback(manifest, request, opts, failure_reason) do
    allow_fallback = Keyword.get(opts, :allow_provider_fallback, false)
    fallback_adapter = Keyword.get(opts, :fallback_adapter)

    if allow_fallback and is_atom(fallback_adapter) do
      fallback_provider = normalize_provider(Keyword.get(opts, :fallback_provider, "fallback"))

      case call_provider(fallback_adapter, fallback_provider, request, manifest, opts) do
        {:ok, provider_result} ->
          {:ok,
           provider_result
           |> Map.put("fallback_used", true)
           |> Map.put("fallback_provider", fallback_provider)
           |> Map.put("primary_provider", normalize_provider(manifest["provider"]))
           |> Map.put("fallback_reason", to_string(failure_reason))}

        {:error, _reason} ->
          {:error, :fallback_exhausted}
      end
    else
      {:error, :no_fallback}
    end
  end

  defp breaker_opts(opts) do
    [
      failure_threshold: Keyword.get(opts, :provider_failure_threshold, 3),
      reset_timeout_ms: Keyword.get(opts, :provider_reset_timeout_ms, 30_000)
    ]
  end

  defp normalize_provider(value) when is_binary(value) do
    value
    |> String.trim()
    |> case do
      "" -> "unknown"
      normalized -> normalized
    end
  end

  defp normalize_provider(_), do: "unknown"

  defp evaluate_policy_gate(manifest, request, base_policy, budget, policy_context, opts) do
    cond do
      consent_denied?(manifest, request, opts) ->
        {:blocked,
         deny_policy(
           base_policy,
           budget,
           policy_context,
           "policy_denied.consent_required"
         )}

      suppressed_recipient?(manifest, request, opts) ->
        {:blocked,
         deny_policy(
           base_policy,
           budget,
           policy_context,
           "policy_denied.suppressed_recipient"
         )}

      true ->
        evaluated = PolicyEvaluator.evaluate(base_policy, budget, policy_context)

        if evaluated["decision"] == "denied" do
          {:blocked, evaluated}
        else
          {:allowed, evaluated}
        end
    end
  end

  defp validate_request(request, manifest) when is_map(request) and is_map(manifest) do
    request = normalize_map(request)

    missing_fields =
      Enum.reject(@required_request_fields, fn field ->
        present_value?(request[field])
      end)

    errors =
      []
      |> maybe_add(
        missing_fields == [],
        "missing required request fields: #{Enum.join(missing_fields, ", ")}"
      )
      |> maybe_add(
        request["integration_id"] == manifest["integration_id"],
        "request.integration_id must match manifest.integration_id"
      )
      |> maybe_add(is_map(request["variables"]), "request.variables must be an object")

    if errors == [] do
      {:ok, request}
    else
      {:error, {:invalid_request, errors}}
    end
  end

  defp consent_denied?(manifest, request, opts) do
    consent_required? =
      manifest
      |> get_in(["policy", "consent_required"])
      |> Kernel.==(true)

    consent_granted =
      cond do
        is_boolean(request["consent_granted"]) ->
          request["consent_granted"]

        is_boolean(Keyword.get(opts, :consent_granted)) ->
          Keyword.get(opts, :consent_granted)

        true ->
          false
      end

    consent_required? and not consent_granted
  end

  defp suppressed_recipient?(manifest, request, opts) do
    suppression_mode =
      manifest
      |> get_in(["policy", "suppression_mode"])
      |> to_string()

    suppressed_recipients =
      case Keyword.get(opts, :suppressed_recipients, []) do
        %MapSet{} = set -> MapSet.to_list(set)
        list when is_list(list) -> list
        nil -> []
        other -> [other]
      end
      |> Enum.map(&to_string/1)
      |> MapSet.new()

    suppression_mode == "enforce" and MapSet.member?(suppressed_recipients, request["recipient"])
  end

  defp deny_policy(base_policy, budget, policy_context, reason_code) do
    base_policy
    |> Map.put("decision", "denied")
    |> Map.put("reason_code", reason_code)
    |> PolicyEvaluator.evaluate(budget, policy_context)
  end

  defp build_base_policy(opts) do
    %{
      "policy_id" => Keyword.get(opts, :policy_id, "comms.send.v1"),
      "authorization_id" => Keyword.get(opts, :authorization_id),
      "authorization_mode" => Keyword.get(opts, :authorization_mode, "delegated_budget")
    }
  end

  defp build_outcome(
         manifest,
         request,
         policy_eval,
         state,
         message_id,
         authorization_id,
         provider_result,
         opts \\ []
       )

  defp build_outcome(
         manifest,
         request,
         policy_eval,
         state,
         message_id,
         authorization_id,
         provider_result,
         opts
       ) do
    sanitized_request = Sanitizer.sanitize(request)
    sanitized_provider_result = Sanitizer.sanitize(provider_result)
    fallback = Keyword.get(opts, :fallback)
    provider = sanitized_provider_result["provider"] || manifest["provider"]
    reason_code = normalize_reason_code(policy_eval["reason_code"], "policy_denied.explicit_deny")
    decision = if(state in ["blocked", "failed"], do: "denied", else: "allowed")

    receipt =
      %{
        "receipt_id" =>
          "comms_" <>
            String.slice(
              Receipts.stable_hash(%{
                "integration_id" => manifest["integration_id"],
                "recipient" => sanitized_request["recipient"],
                "template_id" => sanitized_request["template_id"],
                "state" => state,
                "reason_code" => reason_code,
                "evaluation_hash" => policy_eval["evaluation_hash"]
              }),
              0,
              24
            ),
        "integration_id" => manifest["integration_id"],
        "provider" => provider,
        "recipient" => sanitized_request["recipient"],
        "template_id" => sanitized_request["template_id"],
        "state" => state,
        "reason_code" => reason_code,
        "authorization_id" => authorization_id || "auth_missing",
        "decision" => decision,
        "evaluation_hash" => policy_eval["evaluation_hash"]
      }
      |> maybe_put("message_id", message_id)
      |> maybe_put("fallback", fallback)

    outcome =
      %{
        "state" => state,
        "message_id" => message_id,
        "reason_code" => reason_code,
        "decision" => decision,
        "policy" => policy_eval,
        "provider_result" => sanitized_provider_result,
        "receipt" => receipt
      }
      |> maybe_put("fallback", fallback)

    case Layer0TypeAdapters.comms_send_result(outcome) do
      {:ok, _result} ->
        outcome

      {:error, errors} ->
        raise ArgumentError,
              "layer0 comms_send_result adapter validation failed: #{Enum.join(errors, "; ")}"
    end
  end

  defp present_value?(value) when is_binary(value), do: String.trim(value) != ""
  defp present_value?(%{}), do: true
  defp present_value?(value), do: not is_nil(value)

  defp maybe_add(errors, true, _message), do: errors
  defp maybe_add(errors, false, message), do: [message | errors]

  defp normalize_map(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp normalize_map(_), do: %{}

  defp normalize_reason_code(reason_code, fallback) do
    reason_code = to_string(reason_code || "")

    if PolicyReasonCodes.valid?(reason_code) do
      reason_code
    else
      fallback
    end
  end

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)

  defp validate_comms_intent(manifest, request, opts) do
    case Layer0TypeAdapters.comms_send_intent(manifest, request, opts) do
      {:ok, _intent} -> {:ok, :validated}
      {:error, errors} -> {:error, {:invalid_comms_intent, errors}}
    end
  end
end
