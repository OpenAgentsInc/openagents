defmodule OpenAgentsRuntime.Contracts.RuntimeOrchestrationProtoContract do
  @moduledoc """
  Validates runtime orchestration proto and fixture convergence.
  """

  @repo_root Path.expand("../../../../..", __DIR__)
  @proto_path Path.join(@repo_root, "proto/openagents/runtime/v1/orchestration.proto")

  @fixture_path Path.join(
                  @repo_root,
                  "docs/protocol/fixtures/runtime-orchestration-v1.json"
                )

  @required_messages [
    "RuntimeRun",
    "RuntimeRunLifecycleTransition",
    "RuntimeRunEvent",
    "RuntimeWorker",
    "RuntimeWorkerEvent",
    "RuntimePredictReceipt",
    "RuntimeReplayArtifact",
    "RuntimeResultEnvelope"
  ]

  @required_run_statuses [
    "RUNTIME_RUN_STATUS_CREATED",
    "RUNTIME_RUN_STATUS_RUNNING",
    "RUNTIME_RUN_STATUS_CANCELING",
    "RUNTIME_RUN_STATUS_CANCELED",
    "RUNTIME_RUN_STATUS_SUCCEEDED",
    "RUNTIME_RUN_STATUS_FAILED"
  ]

  @required_worker_statuses [
    "RUNTIME_WORKER_STATUS_RUNNING",
    "RUNTIME_WORKER_STATUS_STOPPED",
    "RUNTIME_WORKER_STATUS_FAILED"
  ]

  @required_error_codes [
    "RUNTIME_ERROR_CODE_UNAUTHORIZED",
    "RUNTIME_ERROR_CODE_FORBIDDEN",
    "RUNTIME_ERROR_CODE_INVALID_REQUEST",
    "RUNTIME_ERROR_CODE_NOT_FOUND",
    "RUNTIME_ERROR_CODE_CONFLICT",
    "RUNTIME_ERROR_CODE_STALE_CURSOR",
    "RUNTIME_ERROR_CODE_INTERNAL_ERROR"
  ]

  @required_run_event_payload_fields [
    "run_started",
    "text_delta",
    "tool_call",
    "tool_result",
    "run_finished",
    "run_cancel_requested"
  ]

  @required_worker_event_payload_fields [
    "worker_started",
    "worker_request_received",
    "worker_response",
    "worker_error",
    "worker_heartbeat",
    "worker_stopped"
  ]

  @allowed_run_transitions MapSet.new([
                             {"RUNTIME_RUN_STATUS_CREATED", "RUNTIME_RUN_STATUS_RUNNING"},
                             {"RUNTIME_RUN_STATUS_RUNNING", "RUNTIME_RUN_STATUS_CANCELING"},
                             {"RUNTIME_RUN_STATUS_RUNNING", "RUNTIME_RUN_STATUS_SUCCEEDED"},
                             {"RUNTIME_RUN_STATUS_RUNNING", "RUNTIME_RUN_STATUS_FAILED"},
                             {"RUNTIME_RUN_STATUS_CANCELING", "RUNTIME_RUN_STATUS_CANCELED"}
                           ])

  @spec check() :: :ok | {:error, [String.t()]}
  def check do
    with {:ok, proto_body} <- read_artifact(@proto_path),
         {:ok, fixture_raw} <- read_artifact(@fixture_path),
         {:ok, fixture} <- decode_json(@fixture_path, fixture_raw) do
      errors =
        []
        |> Kernel.++(validate_required_messages(proto_body))
        |> Kernel.++(validate_enum_values(proto_body, "RuntimeRunStatus", @required_run_statuses))
        |> Kernel.++(
          validate_enum_values(proto_body, "RuntimeWorkerStatus", @required_worker_statuses)
        )
        |> Kernel.++(validate_enum_values(proto_body, "RuntimeErrorCode", @required_error_codes))
        |> Kernel.++(
          validate_oneof_fields(
            proto_body,
            "RuntimeRunEvent",
            "payload",
            @required_run_event_payload_fields
          )
        )
        |> Kernel.++(
          validate_oneof_fields(
            proto_body,
            "RuntimeWorkerEvent",
            "payload",
            @required_worker_event_payload_fields
          )
        )
        |> Kernel.++(validate_fixture(fixture))
        |> Enum.uniq()

      if errors == [], do: :ok, else: {:error, errors}
    else
      {:error, reason} -> {:error, [reason]}
    end
  end

  defp validate_required_messages(proto_body) do
    Enum.reduce(@required_messages, [], fn message_name, errors ->
      if Regex.match?(~r/message\s+#{Regex.escape(message_name)}\s*\{/m, proto_body) do
        errors
      else
        ["runtime orchestration proto missing message '#{message_name}'" | errors]
      end
    end)
  end

  defp validate_enum_values(proto_body, enum_name, expected_values) do
    with {:ok, enum_body} <- extract_enum_body(proto_body, enum_name) do
      actual_values =
        Regex.scan(~r/^\s*([A-Z0-9_]+)\s*=\s*\d+\s*;/m, enum_body)
        |> Enum.map(fn [_full, enum_value] -> enum_value end)
        |> MapSet.new()

      expected_set = MapSet.new(expected_values)

      missing =
        expected_set
        |> MapSet.difference(actual_values)
        |> MapSet.to_list()
        |> Enum.sort()

      if missing == [] do
        []
      else
        ["runtime orchestration enum '#{enum_name}' missing values: #{Enum.join(missing, ", ")}"]
      end
    else
      {:error, :enum_not_found} -> ["runtime orchestration proto missing enum '#{enum_name}'"]
    end
  end

  defp validate_oneof_fields(proto_body, message_name, oneof_name, expected_fields) do
    with {:ok, message_body} <- extract_message_body(proto_body, message_name),
         {:ok, oneof_body} <- extract_oneof_body(message_body, oneof_name) do
      actual_fields =
        Regex.scan(~r/^\s*[A-Za-z0-9_.<>]+\s+([a-zA-Z0-9_]+)\s*=\s*\d+\s*;/m, oneof_body)
        |> Enum.map(fn [_full, field_name] -> field_name end)
        |> MapSet.new()

      missing =
        expected_fields
        |> MapSet.new()
        |> MapSet.difference(actual_fields)
        |> MapSet.to_list()
        |> Enum.sort()

      if missing == [] do
        []
      else
        [
          "runtime orchestration oneof '#{message_name}.#{oneof_name}' missing fields: #{Enum.join(missing, ", ")}"
        ]
      end
    else
      {:error, :message_not_found} ->
        ["runtime orchestration proto missing message '#{message_name}'"]

      {:error, :oneof_not_found} ->
        ["runtime orchestration proto missing oneof '#{message_name}.#{oneof_name}'"]
    end
  end

  defp validate_fixture(fixture) when is_map(fixture) do
    []
    |> Kernel.++(validate_run_transitions(fixture["run_lifecycle_transitions"]))
    |> Kernel.++(validate_worker_events(fixture["worker_lifecycle_events"]))
    |> Kernel.++(validate_runtime_receipt(fixture["runtime_predict_receipt"]))
    |> Kernel.++(validate_replay_artifact(fixture["runtime_replay_artifact"]))
    |> Kernel.++(validate_result_envelopes(fixture["result_envelope_examples"]))
  end

  defp validate_fixture(_fixture), do: ["runtime orchestration fixture must be a JSON object"]

  defp validate_run_transitions(transitions) when is_list(transitions) do
    errors =
      Enum.with_index(transitions, 1)
      |> Enum.reduce([], fn {transition, index}, acc ->
        if is_map(transition) do
          from_status = transition["from_status"]
          to_status = transition["to_status"]
          run_id = transition["run_id"]
          event_type = transition["transition_event_type"]
          latest_seq = transition["latest_seq"]

          acc
          |> maybe_add(
            is_binary(run_id) and String.trim(run_id) != "",
            "runtime transition ##{index} missing run_id"
          )
          |> maybe_add(
            is_binary(event_type) and String.trim(event_type) != "",
            "runtime transition ##{index} missing transition_event_type"
          )
          |> maybe_add(
            from_status in @required_run_statuses,
            "runtime transition ##{index} has invalid from_status: #{inspect(from_status)}"
          )
          |> maybe_add(
            to_status in @required_run_statuses,
            "runtime transition ##{index} has invalid to_status: #{inspect(to_status)}"
          )
          |> maybe_add(
            MapSet.member?(@allowed_run_transitions, {from_status, to_status}),
            "runtime transition ##{index} has unsupported transition: #{from_status} -> #{to_status}"
          )
          |> maybe_add(
            is_integer(latest_seq) and latest_seq >= 0,
            "runtime transition ##{index} has invalid latest_seq"
          )
        else
          ["runtime transition ##{index} must be an object" | acc]
        end
      end)

    if transitions == [] do
      ["runtime orchestration fixture missing run_lifecycle_transitions"] ++ errors
    else
      errors
    end
  end

  defp validate_run_transitions(_transitions),
    do: ["runtime orchestration fixture run_lifecycle_transitions must be an array"]

  defp validate_worker_events(events) when is_list(events) do
    errors =
      Enum.with_index(events, 1)
      |> Enum.reduce([], fn {event, index}, acc ->
        if is_map(event) do
          acc
          |> maybe_add(
            is_binary(event["worker_id"]) and String.trim(event["worker_id"]) != "",
            "worker lifecycle event ##{index} missing worker_id"
          )
          |> maybe_add(
            is_binary(event["event_type"]) and String.trim(event["event_type"]) != "",
            "worker lifecycle event ##{index} missing event_type"
          )
          |> maybe_add(
            event["status"] in @required_worker_statuses,
            "worker lifecycle event ##{index} has invalid status: #{inspect(event["status"])}"
          )
          |> maybe_add(
            is_integer(event["seq"]) and event["seq"] >= 0,
            "worker lifecycle event ##{index} has invalid seq"
          )
        else
          ["worker lifecycle event ##{index} must be an object" | acc]
        end
      end)

    if events == [] do
      ["runtime orchestration fixture missing worker_lifecycle_events"] ++ errors
    else
      errors
    end
  end

  defp validate_worker_events(_events),
    do: ["runtime orchestration fixture worker_lifecycle_events must be an array"]

  defp validate_runtime_receipt(receipt) when is_map(receipt) do
    []
    |> maybe_add(
      is_binary(receipt["receipt_id"]) and String.trim(receipt["receipt_id"]) != "",
      "runtime_predict_receipt missing receipt_id"
    )
    |> maybe_add(
      is_binary(receipt["run_id"]) and String.trim(receipt["run_id"]) != "",
      "runtime_predict_receipt missing run_id"
    )
    |> maybe_add(
      is_map(receipt["policy"]) and
        is_binary(receipt["policy"]["reason_code"]) and
        String.starts_with?(receipt["policy"]["reason_code"], "REASON_CODE_"),
      "runtime_predict_receipt policy.reason_code must use proto reason enum name"
    )
    |> maybe_add(
      is_map(receipt["budget"]) and is_integer(receipt["budget"]["total_tokens"]),
      "runtime_predict_receipt budget.total_tokens must be present"
    )
  end

  defp validate_runtime_receipt(_receipt),
    do: ["runtime orchestration fixture missing runtime_predict_receipt"]

  defp validate_replay_artifact(artifact) when is_map(artifact) do
    first_seq = artifact["first_seq"]
    last_seq = artifact["last_seq"]

    []
    |> maybe_add(
      is_binary(artifact["artifact_id"]) and String.trim(artifact["artifact_id"]) != "",
      "runtime_replay_artifact missing artifact_id"
    )
    |> maybe_add(
      is_binary(artifact["run_id"]) and String.trim(artifact["run_id"]) != "",
      "runtime_replay_artifact missing run_id"
    )
    |> maybe_add(
      is_integer(first_seq) and is_integer(last_seq) and first_seq >= 0 and last_seq >= first_seq,
      "runtime_replay_artifact has invalid first_seq/last_seq ordering"
    )
    |> maybe_add(
      is_integer(artifact["event_count"]) and artifact["event_count"] >= 0,
      "runtime_replay_artifact has invalid event_count"
    )
  end

  defp validate_replay_artifact(_artifact),
    do: ["runtime orchestration fixture missing runtime_replay_artifact"]

  defp validate_result_envelopes(results) when is_list(results) do
    errors =
      Enum.with_index(results, 1)
      |> Enum.reduce([], fn {result, index}, acc ->
        if is_map(result) do
          case result["ok"] do
            true ->
              acc
              |> maybe_add(
                is_binary(result["result_type"]) and String.trim(result["result_type"]) != "",
                "result envelope ##{index} missing result_type for ok=true"
              )

            false ->
              error = result["error"]

              acc
              |> maybe_add(is_map(error), "result envelope ##{index} missing error for ok=false")
              |> maybe_add(
                is_map(error) and error["code"] in @required_error_codes,
                "result envelope ##{index} has invalid runtime error code"
              )

            _other ->
              ["result envelope ##{index} missing boolean ok field" | acc]
          end
        else
          ["result envelope ##{index} must be an object" | acc]
        end
      end)

    if results == [] do
      ["runtime orchestration fixture missing result_envelope_examples"] ++ errors
    else
      errors
    end
  end

  defp validate_result_envelopes(_results),
    do: ["runtime orchestration fixture result_envelope_examples must be an array"]

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

  defp extract_enum_body(proto_body, enum_name) do
    pattern = ~r/enum\s+#{Regex.escape(enum_name)}\s*\{(?<body>.*?)\n\}/ms

    case Regex.named_captures(pattern, proto_body) do
      %{"body" => body} -> {:ok, body}
      _ -> {:error, :enum_not_found}
    end
  end

  defp maybe_add(errors, true, _message), do: errors
  defp maybe_add(errors, false, message), do: [message | errors]
end
