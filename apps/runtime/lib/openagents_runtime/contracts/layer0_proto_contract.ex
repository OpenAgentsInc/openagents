defmodule OpenAgentsRuntime.Contracts.Layer0ProtoContract do
  @moduledoc """
  Validates convergence between runtime payload mappings and Layer-0 proto artifacts.
  """

  alias OpenAgentsRuntime.DS.PolicyReasonCodes
  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.Integrations.LaravelEventMapper

  @repo_root Path.expand("../../../../..", __DIR__)
  @events_proto_path Path.join(@repo_root, "proto/openagents/protocol/v1/events.proto")
  @receipts_proto_path Path.join(@repo_root, "proto/openagents/protocol/v1/receipts.proto")
  @reasons_proto_path Path.join(@repo_root, "proto/openagents/protocol/v1/reasons.proto")
  @reason_json_path Path.join(
                      @repo_root,
                      "docs/protocol/reasons/runtime-policy-reason-codes.v1.json"
                    )

  @runtime_event_oneof_mapping %{
    "run.started" => "run_started",
    "run.delta" => "text_delta",
    "text.delta" => "text_delta",
    "tool.call" => "tool_call",
    "tool.result" => "tool_result",
    "run.finished" => "run_finished"
  }

  @runtime_event_payloads %{
    "run.started" => %{"actor" => "assistant"},
    "run.delta" => %{"delta" => "hello"},
    "text.delta" => %{"delta" => "hello"},
    "tool.call" => %{"tool_call_id" => "tool_1", "tool_name" => "web.search"},
    "tool.result" => %{"tool_call_id" => "tool_1", "state" => "succeeded"},
    "run.finished" => %{"status" => "succeeded"}
  }

  @runtime_event_sse_types %{
    "run.started" => "start",
    "run.delta" => "text-delta",
    "text.delta" => "text-delta",
    "tool.call" => "tool-call",
    "tool.result" => "tool-result",
    "run.finished" => "finish"
  }

  @spec check() :: :ok | {:error, [String.t()]}
  def check do
    with {:ok, events_proto} <- read_artifact(@events_proto_path),
         {:ok, receipts_proto} <- read_artifact(@receipts_proto_path),
         {:ok, reasons_proto} <- read_artifact(@reasons_proto_path),
         {:ok, reason_json_raw} <- read_artifact(@reason_json_path),
         {:ok, reason_json} <- decode_json(@reason_json_path, reason_json_raw) do
      errors =
        []
        |> Kernel.++(validate_runtime_event_mapping(events_proto))
        |> Kernel.++(validate_predict_receipt_mapping(receipts_proto))
        |> Kernel.++(validate_reason_compatibility(events_proto, reasons_proto, reason_json))
        |> Enum.uniq()

      if errors == [], do: :ok, else: {:error, errors}
    else
      {:error, reason} -> {:error, [reason]}
    end
  end

  defp read_artifact(path) do
    if File.exists?(path) do
      case File.read(path) do
        {:ok, body} -> {:ok, body}
        {:error, reason} -> {:error, "failed to read artifact #{path}: #{inspect(reason)}"}
      end
    else
      {:error, "missing artifact: #{path}"}
    end
  end

  defp decode_json(path, body) do
    case Jason.decode(body) do
      {:ok, decoded} when is_map(decoded) -> {:ok, decoded}
      {:ok, _other} -> {:error, "artifact is not a JSON object: #{path}"}
      {:error, reason} -> {:error, "invalid JSON in #{path}: #{inspect(reason)}"}
    end
  end

  defp validate_runtime_event_mapping(events_proto) do
    oneof_fields = extract_run_event_oneof_fields(events_proto) |> MapSet.new()

    errors =
      Enum.reduce(@runtime_event_oneof_mapping, [], fn {event_type, oneof_field}, acc ->
        if MapSet.member?(oneof_fields, oneof_field) do
          acc
        else
          [
            "events.proto RunEvent.oneof payload missing mapped field '#{oneof_field}' for runtime event '#{event_type}'"
            | acc
          ]
        end
      end)

    errors =
      if MapSet.member?(oneof_fields, "unknown") do
        errors
      else
        ["events.proto RunEvent.oneof payload missing 'unknown' fallback field" | errors]
      end

    Enum.reduce(@runtime_event_payloads, errors, fn {event_type, payload}, acc ->
      frames = LaravelEventMapper.map_runtime_event("run_contract", 42, event_type, payload)
      first_frame = List.first(frames)
      expected_sse_type = @runtime_event_sse_types[event_type]

      acc
      |> maybe_add(
        is_map(first_frame) and first_frame.event == "message",
        "LaravelEventMapper did not emit message frame for '#{event_type}'"
      )
      |> validate_sse_payload_type(event_type, first_frame, expected_sse_type)
      |> validate_done_frame(event_type, frames)
    end)
  end

  defp validate_sse_payload_type(errors, _event_type, nil, _expected_sse_type), do: errors

  defp validate_sse_payload_type(errors, event_type, frame, expected_sse_type) do
    with true <- is_binary(frame.data),
         true <- frame.data != "[DONE]",
         {:ok, payload} <- Jason.decode(frame.data) do
      maybe_add(
        errors,
        payload["type"] == expected_sse_type,
        "LaravelEventMapper emitted '#{payload["type"]}' for '#{event_type}', expected '#{expected_sse_type}'"
      )
    else
      false ->
        ["LaravelEventMapper emitted invalid data payload for '#{event_type}'" | errors]

      {:error, _reason} ->
        ["LaravelEventMapper emitted non-JSON payload for '#{event_type}'" | errors]
    end
  end

  defp validate_done_frame(errors, "run.finished", frames) do
    done? =
      frames
      |> Enum.any?(fn frame -> is_map(frame) and frame.data == "[DONE]" end)

    maybe_add(
      errors,
      done?,
      "LaravelEventMapper did not emit [DONE] terminal frame for run.finished"
    )
  end

  defp validate_done_frame(errors, _event_type, _frames), do: errors

  defp validate_predict_receipt_mapping(receipts_proto) do
    predict_receipt_fields =
      extract_message_fields(receipts_proto, "PredictReceipt") |> MapSet.new()

    runtime_receipt_fields =
      Receipts.build_predict(%{
        run_id: "run_contract",
        signature_id: "sig_contract.v1",
        strategy_id: "direct.v1",
        compiled_id: "compiled_contract",
        policy: %{
          "decision" => "allowed",
          "reason_code" => "policy_allowed.default",
          "reason_codes_version" => PolicyReasonCodes.version(),
          "evaluation_hash" => String.duplicate("a", 64)
        },
        budget: %{"spent_sats" => 1, "remaining_sats" => 99},
        timing: %{"latency_ms" => 10}
      })
      |> Map.keys()
      |> Enum.map(&to_string/1)
      |> MapSet.new()

    missing_fields =
      runtime_receipt_fields
      |> MapSet.difference(predict_receipt_fields)
      |> MapSet.to_list()
      |> Enum.sort()

    errors =
      if missing_fields == [] do
        []
      else
        [
          "receipts.proto PredictReceipt missing runtime receipt fields: #{Enum.join(missing_fields, ", ")}"
        ]
      end

    policy_fields = extract_message_fields(receipts_proto, "PolicyDecision") |> MapSet.new()

    errors
    |> maybe_add(
      MapSet.member?(policy_fields, "reason_code"),
      "receipts.proto PolicyDecision missing reason_code"
    )
    |> maybe_add(
      MapSet.member?(policy_fields, "reason_code_text"),
      "receipts.proto PolicyDecision missing reason_code_text"
    )
    |> maybe_add(
      MapSet.member?(policy_fields, "reason_codes_version"),
      "receipts.proto PolicyDecision missing reason_codes_version"
    )
  end

  defp validate_reason_compatibility(events_proto, reasons_proto, reason_json) do
    errors = []

    errors =
      errors
      |> maybe_add(
        message_field_type?(events_proto, "ToolResultPayload", "ReasonCode", "reason_code"),
        "events.proto ToolResultPayload.reason_code must be typed as ReasonCode"
      )
      |> maybe_add(
        message_field_type?(events_proto, "RunFinishedPayload", "ReasonCode", "reason_code"),
        "events.proto RunFinishedPayload.reason_code must be typed as ReasonCode"
      )

    json_domains =
      reason_json["domains"]
      |> List.wrap()
      |> Enum.reject(&is_nil/1)
      |> Enum.sort_by(&String.length/1, :desc)

    json_codes =
      reason_json["reason_codes"]
      |> List.wrap()
      |> Enum.map(& &1["code"])
      |> Enum.reject(&is_nil/1)
      |> MapSet.new()

    proto_reason_codes =
      reasons_proto
      |> extract_reason_enum_names()
      |> Enum.reject(&(&1 == "REASON_CODE_UNSPECIFIED"))
      |> Enum.map(&enum_name_to_reason_code(&1, json_domains))
      |> MapSet.new()

    runtime_reason_codes = PolicyReasonCodes.all() |> MapSet.new()

    errors
    |> maybe_add(
      MapSet.equal?(json_codes, proto_reason_codes),
      mismatch_message("reasons.proto enum vs canonical JSON", json_codes, proto_reason_codes)
    )
    |> maybe_add(
      MapSet.equal?(json_codes, runtime_reason_codes),
      mismatch_message(
        "runtime reason module vs canonical JSON",
        json_codes,
        runtime_reason_codes
      )
    )
  end

  defp extract_run_event_oneof_fields(events_proto) do
    with {:ok, run_event_body} <- extract_message_body(events_proto, "RunEvent"),
         {:ok, payload_body} <- extract_oneof_body(run_event_body, "payload") do
      Regex.scan(~r/^\s*[A-Za-z0-9_.<>]+\s+([a-zA-Z0-9_]+)\s*=\s*\d+\s*;/m, payload_body)
      |> Enum.map(fn [_full, field_name] -> field_name end)
    else
      _ -> []
    end
  end

  defp extract_message_fields(proto_body, message_name) do
    with {:ok, message_body} <- extract_message_body(proto_body, message_name) do
      Regex.scan(~r/^\s*[A-Za-z0-9_.<>]+\s+([a-zA-Z0-9_]+)\s*=\s*\d+\s*;/m, message_body)
      |> Enum.map(fn [_full, field_name] -> field_name end)
    else
      _ -> []
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

  defp message_field_type?(proto_body, message_name, field_type, field_name) do
    case extract_message_body(proto_body, message_name) do
      {:ok, message_body} ->
        Regex.match?(
          ~r/^\s*#{Regex.escape(field_type)}\s+#{Regex.escape(field_name)}\s*=\s*\d+\s*;/m,
          message_body
        )

      {:error, _reason} ->
        false
    end
  end

  defp extract_reason_enum_names(proto_body) when is_binary(proto_body) do
    Regex.scan(~r/^\s*(REASON_CODE_[A-Z0-9_]+)\s*=\s*\d+\s*;/m, proto_body)
    |> Enum.map(fn [_full, name] -> name end)
  end

  defp enum_name_to_reason_code(name, json_domains) when is_binary(name) do
    normalized = name |> String.replace_prefix("REASON_CODE_", "") |> String.downcase()

    case Enum.find(json_domains, &String.starts_with?(normalized, &1 <> "_")) do
      nil ->
        raise """
        unable to map proto enum '#{name}' to a canonical reason code domain.
        expected one of domains: #{Enum.join(json_domains, ", ")}
        """

      domain ->
        remainder = String.replace_prefix(normalized, domain <> "_", "")
        domain <> "." <> remainder
    end
  end

  defp mismatch_message(label, expected_set, actual_set) do
    missing = MapSet.difference(expected_set, actual_set) |> MapSet.to_list() |> Enum.sort()
    extra = MapSet.difference(actual_set, expected_set) |> MapSet.to_list() |> Enum.sort()

    "#{label} mismatch. missing=#{inspect(missing)} extra=#{inspect(extra)}"
  end

  defp maybe_add(errors, true, _message), do: errors
  defp maybe_add(errors, false, message), do: [message | errors]
end
