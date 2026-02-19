defmodule OpenAgentsRuntime.Contracts.Layer0TypeAdapters do
  @moduledoc """
  Proto-derived adapter layer for runtime boundary payloads.

  Adapters keep JSON/SSE transport unchanged while validating and shaping payloads
  to Layer-0 contract-compatible maps.
  """

  alias OpenAgentsRuntime.DS.PolicyReasonCodes

  @repo_root Path.expand("../../../../..", __DIR__)
  @events_proto_path Path.join(@repo_root, "proto/openagents/protocol/v1/events.proto")
  @receipts_proto_path Path.join(@repo_root, "proto/openagents/protocol/v1/receipts.proto")
  @reasons_proto_path Path.join(@repo_root, "proto/openagents/protocol/v1/reasons.proto")
  @comms_proto_path Path.join(@repo_root, "proto/openagents/protocol/v1/comms.proto")

  @runtime_event_oneof_mapping %{
    "run.started" => "run_started",
    "run.delta" => "text_delta",
    "text.delta" => "text_delta",
    "tool.call" => "tool_call",
    "tool.result" => "tool_result",
    "run.finished" => "run_finished"
  }

  @type validation_error :: String.t()
  @type adapter_result :: {:ok, map()} | {:error, [validation_error()]}

  @spec run_event(String.t(), non_neg_integer(), String.t(), map()) :: adapter_result()
  def run_event(run_id, seq, event_type, payload)
      when is_binary(run_id) and is_integer(seq) and is_binary(event_type) and is_map(payload) do
    payload = normalize_map(payload)
    oneof_field = Map.get(@runtime_event_oneof_mapping, event_type, "unknown")

    with {:ok, oneof_payload} <- build_run_event_payload(oneof_field, payload) do
      event_map =
        %{
          "run_id" => run_id,
          "seq" => seq,
          "event_type" => event_type,
          "emitted_at" =>
            DateTime.utc_now() |> DateTime.truncate(:second) |> DateTime.to_iso8601(),
          oneof_field => oneof_payload
        }
        |> maybe_put_unknown(oneof_field, payload)

      validate_run_event_map(event_map, oneof_field)
    end
  end

  @spec predict_receipt(map()) :: adapter_result()
  def predict_receipt(receipt) when is_map(receipt) do
    receipt = normalize_map(receipt)
    policy = normalize_map(receipt["policy"] || %{})

    with {:ok, reason_enum} <- to_reason_enum(policy["reason_code"]),
         {:ok, policy_map} <- adapt_policy(policy, receipt, reason_enum) do
      map =
        %{
          "receipt_id" => to_string(receipt["receipt_id"] || ""),
          "run_id" => to_string(receipt["run_id"] || ""),
          "signature_id" => to_string(receipt["signature_id"] || ""),
          "strategy_id" => to_string(receipt["strategy_id"] || ""),
          "compiled_id" => to_string(receipt["compiled_id"] || ""),
          "schema_hash" => to_string(receipt["schema_hash"] || ""),
          "prompt_hash" => to_string(receipt["prompt_hash"] || ""),
          "program_hash" => to_string(receipt["program_hash"] || ""),
          "params_hash" => to_string(receipt["params_hash"] || ""),
          "output_hash" => to_string(receipt["output_hash"] || ""),
          "policy" => policy_map,
          "budget" => normalize_map(receipt["budget"] || %{}),
          "timing" => normalize_map(receipt["timing"] || %{}),
          "catalog_version" => to_string(receipt["catalog_version"] || ""),
          "trace_ref" => to_string(receipt["trace_ref"] || ""),
          "trace_hash" => to_string(receipt["trace_hash"] || ""),
          "trace_storage" => to_string(receipt["trace_storage"] || ""),
          "trace_artifact_uri" => to_string(receipt["trace_artifact_uri"] || ""),
          "output_preview" => normalize_map(receipt["output_preview"] || %{})
        }

      validate_message("receipts", "PredictReceipt", map)
    end
  end

  @spec comms_send_intent(map(), map(), keyword()) :: adapter_result()
  def comms_send_intent(manifest, request, opts \\ [])
      when is_map(manifest) and is_map(request) and is_list(opts) do
    manifest = normalize_map(manifest)
    request = normalize_map(request)

    map =
      %{
        "org_id" =>
          normalize_optional_string(request["org_id"] || Keyword.get(opts, :org_id)) ||
            "unknown_org",
        "user_id" =>
          normalize_optional_string(request["user_id"] || Keyword.get(opts, :user_id)) ||
            "unknown_user",
        "channel" => normalize_optional_string(request["channel"]) || "email",
        "provider" => normalize_optional_string(manifest["provider"]) || "unknown",
        "template_id" => normalize_optional_string(request["template_id"]) || "",
        "recipient" => normalize_optional_string(request["recipient"]) || "",
        "variables" => normalize_map(request["variables"] || %{})
      }

    validate_message("comms", "CommsSendIntent", map)
  end

  @spec comms_send_result(map()) :: adapter_result()
  def comms_send_result(outcome) when is_map(outcome) do
    outcome = normalize_map(outcome)
    reason_code = outcome["reason_code"] || "policy_denied.explicit_deny"

    with {:ok, reason_enum} <- to_reason_enum(reason_code) do
      map =
        %{
          "message_id" => to_string(outcome["message_id"] || ""),
          "state" => to_string(outcome["state"] || "failed"),
          "reason_code" => reason_enum,
          "reason_code_text" => to_string(reason_code),
          "provider_metadata" => normalize_map(outcome["provider_result"] || %{})
        }

      validate_message("comms", "CommsSendResult", map)
    end
  end

  defp build_run_event_payload("run_started", payload),
    do: {:ok, %{"actor" => to_string(payload["actor"] || "runtime")}}

  defp build_run_event_payload("text_delta", payload),
    do: {:ok, %{"delta" => to_string(payload["delta"] || payload["text"] || "")}}

  defp build_run_event_payload("tool_call", payload) do
    {:ok,
     %{
       "tool_call_id" => to_string(payload["tool_call_id"] || "tool_unknown"),
       "tool_name" => to_string(payload["tool_name"] || "tool.unknown"),
       "input" => normalize_map(payload["input"] || %{})
     }}
  end

  defp build_run_event_payload("tool_result", payload) do
    with {:ok, reason_enum} <- to_reason_enum(payload["reason_code"]) do
      map =
        %{
          "tool_call_id" => to_string(payload["tool_call_id"] || "tool_unknown"),
          "tool_name" => to_string(payload["tool_name"] || "tool.unknown"),
          "state" => to_string(payload["state"] || "unknown"),
          "output" => normalize_map(payload["output"] || %{})
        }
        |> maybe_put("reason_code", reason_enum)

      {:ok, map}
    end
  end

  defp build_run_event_payload("run_finished", payload) do
    with {:ok, reason_enum} <- to_reason_enum(payload["reason_code"]) do
      map =
        %{"status" => to_string(payload["status"] || "unknown")}
        |> maybe_put("reason_code", reason_enum)

      {:ok, map}
    end
  end

  defp build_run_event_payload("unknown", _payload), do: {:ok, %{"body" => %{}}}
  defp build_run_event_payload(_field, _payload), do: {:error, ["unsupported run event mapping"]}

  defp adapt_policy(policy, receipt, reason_enum) do
    map =
      %{
        "policy_id" => to_string(policy["policy_id"] || "ds.predict.v1"),
        "authorization_id" => to_string(policy["authorization_id"] || "auth_missing"),
        "authorization_mode" => to_string(policy["authorization_mode"] || "delegated_budget"),
        "decision" => to_string(policy["decision"] || "denied"),
        "reason_code" => reason_enum || "REASON_CODE_UNSPECIFIED",
        "reason_code_text" => to_string(policy["reason_code"] || "policy_denied.explicit_deny"),
        "reason_codes_version" =>
          to_string(policy["reason_codes_version"] || PolicyReasonCodes.version()),
        "evaluation_hash" => to_string(policy["evaluation_hash"] || String.duplicate("0", 64)),
        "compiled_id" => to_string(policy["compiled_id"] || receipt["compiled_id"] || ""),
        "strategy_id" => to_string(policy["strategy_id"] || receipt["strategy_id"] || ""),
        "artifact_variant" => to_string(policy["artifact_variant"] || "primary"),
        "canary_percent" => normalize_int(policy["canary_percent"], 0),
        "rollout_bucket" => normalize_int(policy["rollout_bucket"], 0)
      }

    validate_message("receipts", "PolicyDecision", map)
  end

  defp maybe_put_unknown(map, "unknown", payload),
    do: Map.put(map, "unknown", %{"body" => payload})

  defp maybe_put_unknown(map, _field, _payload), do: map

  defp validate_run_event_map(map, oneof_field) do
    base_fields = MapSet.new(["run_id", "seq", "event_type", "emitted_at"])
    keys = Map.keys(map) |> MapSet.new()
    oneof_fields = run_event_oneof_fields()

    errors =
      []
      |> maybe_add(
        MapSet.subset?(base_fields, keys),
        "events.proto RunEvent missing base fields"
      )
      |> maybe_add(
        MapSet.member?(oneof_fields, oneof_field),
        "events.proto RunEvent missing oneof payload field '#{oneof_field}'"
      )

    if errors == [], do: {:ok, map}, else: {:error, errors}
  end

  defp to_reason_enum(nil), do: {:ok, nil}
  defp to_reason_enum(""), do: {:ok, nil}

  defp to_reason_enum(reason_code) when is_binary(reason_code) do
    enum_name =
      "REASON_CODE_" <>
        (reason_code
         |> String.replace(".", "_")
         |> String.upcase())

    proto_reasons = proto_reason_enums()

    if MapSet.member?(proto_reasons, enum_name) do
      {:ok, enum_name}
    else
      {:error, ["unsupported reason code for proto enum mapping: #{reason_code}"]}
    end
  end

  defp to_reason_enum(reason_code), do: to_reason_enum(to_string(reason_code))

  defp validate_message(domain, message_name, map) when is_map(map) do
    fields = message_fields(domain, message_name)
    keys = Map.keys(map) |> MapSet.new()

    missing =
      fields
      |> MapSet.difference(keys)
      |> MapSet.to_list()
      |> Enum.sort()

    errors =
      if missing == [] do
        []
      else
        ["#{domain}.proto #{message_name} missing fields: #{Enum.join(missing, ", ")}"]
      end

    if errors == [], do: {:ok, map}, else: {:error, errors}
  end

  defp message_fields("events", "RunEvent"),
    do: cached_message_fields(@events_proto_path, "RunEvent")

  defp message_fields("receipts", "PredictReceipt"),
    do: cached_message_fields(@receipts_proto_path, "PredictReceipt")

  defp message_fields("receipts", "PolicyDecision"),
    do: cached_message_fields(@receipts_proto_path, "PolicyDecision")

  defp message_fields("comms", "CommsSendIntent"),
    do: cached_message_fields(@comms_proto_path, "CommsSendIntent")

  defp message_fields("comms", "CommsSendResult"),
    do: cached_message_fields(@comms_proto_path, "CommsSendResult")

  defp message_fields(_domain, _message), do: MapSet.new()

  defp cached_message_fields(proto_path, message_name) do
    cache_key = {:message_fields, proto_path, message_name}

    case :persistent_term.get({__MODULE__, cache_key}, :missing) do
      :missing ->
        value = extract_message_fields(proto_path, message_name)
        :persistent_term.put({__MODULE__, cache_key}, value)
        value

      value ->
        value
    end
  end

  defp proto_reason_enums do
    cache_key = {:reason_enums, @reasons_proto_path}

    case :persistent_term.get({__MODULE__, cache_key}, :missing) do
      :missing ->
        value =
          @reasons_proto_path
          |> File.read!()
          |> then(&Regex.scan(~r/^\s*(REASON_CODE_[A-Z0-9_]+)\s*=\s*\d+\s*;/m, &1))
          |> Enum.map(fn [_full, enum_name] -> enum_name end)
          |> MapSet.new()

        :persistent_term.put({__MODULE__, cache_key}, value)
        value

      value ->
        value
    end
  end

  defp run_event_oneof_fields do
    cache_key = {:run_event_oneof_fields, @events_proto_path}

    case :persistent_term.get({__MODULE__, cache_key}, :missing) do
      :missing ->
        value =
          case File.read(@events_proto_path) do
            {:ok, proto_body} ->
              with {:ok, run_event_body} <- extract_message_body(proto_body, "RunEvent"),
                   {:ok, oneof_body} <- extract_oneof_body(run_event_body, "payload") do
                Regex.scan(
                  ~r/^\s*[A-Za-z0-9_.<>]+\s+([a-zA-Z0-9_]+)\s*=\s*\d+\s*;/m,
                  oneof_body
                )
                |> Enum.map(fn [_full, field_name] -> field_name end)
                |> MapSet.new()
              else
                _ -> MapSet.new()
              end

            {:error, _reason} ->
              MapSet.new()
          end

        :persistent_term.put({__MODULE__, cache_key}, value)
        value

      value ->
        value
    end
  end

  defp extract_message_fields(proto_path, message_name) do
    case File.read(proto_path) do
      {:ok, proto_body} ->
        with {:ok, message_body} <- extract_message_body(proto_body, message_name) do
          Regex.scan(~r/^\s*[A-Za-z0-9_.<>]+\s+([a-zA-Z0-9_]+)\s*=\s*\d+\s*;/m, message_body)
          |> Enum.map(fn [_full, field_name] -> field_name end)
          |> MapSet.new()
        else
          {:error, _reason} -> MapSet.new()
        end

      {:error, _reason} ->
        MapSet.new()
    end
  end

  defp extract_message_body(proto_body, message_name) do
    pattern = ~r/message\s+#{Regex.escape(message_name)}\s*\{(?<body>.*?)\n\}/ms

    case Regex.named_captures(pattern, proto_body) do
      %{"body" => body} -> {:ok, body}
      _ -> {:error, :message_not_found}
    end
  end

  defp extract_oneof_body(message_body, oneof_name) do
    pattern = ~r/oneof\s+#{Regex.escape(oneof_name)}\s*\{(?<body>.*?)\n\s*\}/ms

    case Regex.named_captures(pattern, message_body) do
      %{"body" => body} -> {:ok, body}
      _ -> {:error, :oneof_not_found}
    end
  end

  defp normalize_map(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), normalize_value(value)}
      {key, value} -> {to_string(key), normalize_value(value)}
    end)
  end

  defp normalize_map(_), do: %{}

  defp normalize_value(value) when is_map(value), do: normalize_map(value)
  defp normalize_value(list) when is_list(list), do: Enum.map(list, &normalize_value/1)
  defp normalize_value(value), do: value

  defp normalize_optional_string(nil), do: nil

  defp normalize_optional_string(value) when is_binary(value) do
    trimmed = String.trim(value)
    if trimmed == "", do: nil, else: trimmed
  end

  defp normalize_optional_string(value), do: value |> to_string() |> normalize_optional_string()

  defp normalize_int(value, _fallback) when is_integer(value), do: value

  defp normalize_int(value, fallback) when is_binary(value) do
    case Integer.parse(value) do
      {parsed, ""} -> parsed
      _ -> fallback
    end
  end

  defp normalize_int(_value, fallback), do: fallback

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)

  defp maybe_add(errors, true, _message), do: errors
  defp maybe_add(errors, false, message), do: [message | errors]
end
