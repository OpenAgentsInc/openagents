defmodule OpenAgentsRuntime.Contracts.CodexProtoContract do
  @moduledoc """
  Validates Codex proto contracts and cross-surface fixture convergence.
  """

  @repo_root Path.expand("../../../../..", __DIR__)
  @events_proto_path Path.join(@repo_root, "proto/openagents/codex/v1/events.proto")
  @workers_proto_path Path.join(@repo_root, "proto/openagents/codex/v1/workers.proto")
  @auth_proto_path Path.join(@repo_root, "proto/openagents/codex/v1/auth.proto")

  @fixture_path Path.join(@repo_root, "docs/protocol/fixtures/codex-worker-events-v1.json")

  @required_event_messages [
    "CodexProjectionEnvelope",
    "CodexWorkerEvent",
    "CodexNotificationEnvelope",
    "CodexReplayMetadata",
    "CodexTokenUsage",
    "CodexStreamError"
  ]

  @required_worker_messages [
    "CodexProjectionStatus",
    "CodexWorker",
    "CodexWorkerSummary",
    "CodexWorkerSnapshot",
    "CodexWorkerCreateResponse",
    "CodexWorkerRequestResponse",
    "CodexWorkerStopResponse"
  ]

  @required_auth_messages [
    "CodexAuthTokenReference",
    "CodexDeviceFlowStatus",
    "CodexAuthStateEnvelope"
  ]

  @required_notification_methods [
    "CODEX_NOTIFICATION_METHOD_THREAD_STARTED",
    "CODEX_NOTIFICATION_METHOD_TURN_STARTED",
    "CODEX_NOTIFICATION_METHOD_ITEM_STARTED",
    "CODEX_NOTIFICATION_METHOD_ITEM_AGENT_MESSAGE_DELTA",
    "CODEX_NOTIFICATION_METHOD_ITEM_REASONING_DELTA",
    "CODEX_NOTIFICATION_METHOD_ITEM_COMPLETED",
    "CODEX_NOTIFICATION_METHOD_TURN_COMPLETED",
    "CODEX_NOTIFICATION_METHOD_IOS_HANDSHAKE",
    "CODEX_NOTIFICATION_METHOD_DESKTOP_HANDSHAKE_ACK",
    "CODEX_NOTIFICATION_METHOD_USER_MESSAGE"
  ]

  @required_stream_error_codes [
    "CODEX_STREAM_ERROR_CODE_UNAUTHORIZED",
    "CODEX_STREAM_ERROR_CODE_FORBIDDEN",
    "CODEX_STREAM_ERROR_CODE_STALE_CURSOR",
    "CODEX_STREAM_ERROR_CODE_REAUTH_REQUIRED",
    "CODEX_STREAM_ERROR_CODE_INTERNAL_ERROR"
  ]

  @required_auth_statuses [
    "CODEX_AUTH_HYDRATION_STATUS_UNAUTHENTICATED",
    "CODEX_AUTH_HYDRATION_STATUS_DEVICE_FLOW_PENDING",
    "CODEX_AUTH_HYDRATION_STATUS_AUTHENTICATED",
    "CODEX_AUTH_HYDRATION_STATUS_EXPIRED",
    "CODEX_AUTH_HYDRATION_STATUS_ERROR"
  ]

  @required_worker_payload_fields [
    "worker_started",
    "worker_request_received",
    "worker_response",
    "worker_error",
    "worker_heartbeat",
    "worker_stopped",
    "unknown"
  ]

  @required_notification_payload_fields [
    "thread_started",
    "turn_started",
    "turn_completed",
    "turn_failure",
    "item_lifecycle",
    "text_delta",
    "tool_output_delta",
    "codex_error",
    "ios_handshake",
    "desktop_handshake_ack",
    "user_message",
    "unknown"
  ]

  @spec check() :: :ok | {:error, [String.t()]}
  def check do
    with {:ok, events_proto} <- read_artifact(@events_proto_path),
         {:ok, workers_proto} <- read_artifact(@workers_proto_path),
         {:ok, auth_proto} <- read_artifact(@auth_proto_path),
         {:ok, fixture_raw} <- read_artifact(@fixture_path),
         {:ok, fixture} <- decode_json(@fixture_path, fixture_raw) do
      errors =
        []
        |> Kernel.++(validate_required_messages(events_proto, @required_event_messages, "events"))
        |> Kernel.++(
          validate_required_messages(workers_proto, @required_worker_messages, "workers")
        )
        |> Kernel.++(validate_required_messages(auth_proto, @required_auth_messages, "auth"))
        |> Kernel.++(
          validate_enum_values(
            events_proto,
            "CodexNotificationMethod",
            @required_notification_methods,
            "events"
          )
        )
        |> Kernel.++(
          validate_enum_values(
            events_proto,
            "CodexStreamErrorCode",
            @required_stream_error_codes,
            "events"
          )
        )
        |> Kernel.++(
          validate_enum_values(
            auth_proto,
            "CodexAuthHydrationStatus",
            @required_auth_statuses,
            "auth"
          )
        )
        |> Kernel.++(
          validate_oneof_fields(
            events_proto,
            "CodexWorkerEvent",
            "payload",
            @required_worker_payload_fields,
            "events"
          )
        )
        |> Kernel.++(
          validate_oneof_fields(
            events_proto,
            "CodexNotificationEnvelope",
            "payload",
            @required_notification_payload_fields,
            "events"
          )
        )
        |> Kernel.++(validate_fixture(fixture))
        |> Enum.uniq()

      if errors == [], do: :ok, else: {:error, errors}
    else
      {:error, reason} -> {:error, [reason]}
    end
  end

  defp validate_required_messages(proto_body, required_messages, label) do
    Enum.reduce(required_messages, [], fn message_name, errors ->
      if Regex.match?(~r/message\s+#{Regex.escape(message_name)}\s*\{/m, proto_body) do
        errors
      else
        ["codex #{label} proto missing message '#{message_name}'" | errors]
      end
    end)
  end

  defp validate_enum_values(proto_body, enum_name, expected_values, label) do
    with {:ok, enum_body} <- extract_enum_body(proto_body, enum_name) do
      actual_values =
        Regex.scan(~r/^\s*([A-Z0-9_]+)\s*=\s*\d+\s*;/m, enum_body)
        |> Enum.map(fn [_full, enum_value] -> enum_value end)
        |> MapSet.new()

      missing =
        expected_values
        |> MapSet.new()
        |> MapSet.difference(actual_values)
        |> MapSet.to_list()
        |> Enum.sort()

      if missing == [] do
        []
      else
        ["codex #{label} enum '#{enum_name}' missing values: #{Enum.join(missing, ", ")}"]
      end
    else
      {:error, :enum_not_found} -> ["codex #{label} proto missing enum '#{enum_name}'"]
    end
  end

  defp validate_oneof_fields(proto_body, message_name, oneof_name, expected_fields, label) do
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
          "codex #{label} oneof '#{message_name}.#{oneof_name}' missing fields: #{Enum.join(missing, ", ")}"
        ]
      end
    else
      {:error, :message_not_found} ->
        ["codex #{label} proto missing message '#{message_name}'"]

      {:error, :oneof_not_found} ->
        ["codex #{label} proto missing oneof '#{message_name}.#{oneof_name}'"]
    end
  end

  defp validate_fixture(fixture) when is_map(fixture) do
    []
    |> Kernel.++(validate_worker_summary(fixture["worker_summary"]))
    |> Kernel.++(validate_worker_snapshot(fixture["worker_snapshot"]))
    |> Kernel.++(validate_create_response(fixture["worker_create_response"]))
    |> Kernel.++(validate_request_response(fixture["worker_request_response"]))
    |> Kernel.++(validate_auth_states(fixture["auth_states"]))
    |> Kernel.++(validate_notifications(fixture["notification_events"]))
    |> Kernel.++(validate_stream_errors(fixture["stream_errors"]))
  end

  defp validate_fixture(_fixture), do: ["codex fixture must be a JSON object"]

  defp validate_worker_summary(summary) when is_map(summary) do
    projection = summary["khala_projection"]

    []
    |> maybe_add(
      is_binary(summary["worker_id"]) and String.trim(summary["worker_id"]) != "",
      "worker_summary missing worker_id"
    )
    |> maybe_add(
      is_binary(summary["status"]) and String.trim(summary["status"]) != "",
      "worker_summary missing status"
    )
    |> maybe_add(
      is_integer(summary["latest_seq"]) and summary["latest_seq"] >= 0,
      "worker_summary latest_seq must be >= 0"
    )
    |> maybe_add(
      is_binary(summary["adapter"]) and String.trim(summary["adapter"]) != "",
      "worker_summary missing adapter"
    )
    |> maybe_add(is_map(projection), "worker_summary missing khala_projection")
    |> maybe_add(
      is_map(projection) and is_binary(projection["document_id"]) and
        String.trim(projection["document_id"]) != "",
      "worker_summary.khala_projection missing document_id"
    )
    |> maybe_add(
      is_map(projection) and is_integer(projection["last_runtime_seq"]) and
        projection["last_runtime_seq"] >= 0,
      "worker_summary.khala_projection last_runtime_seq must be >= 0"
    )
  end

  defp validate_worker_summary(_summary), do: ["codex fixture missing worker_summary"]

  defp validate_worker_snapshot(snapshot) when is_map(snapshot) do
    worker = snapshot["worker"]
    projection = snapshot["projection"]

    []
    |> maybe_add(is_map(worker), "worker_snapshot missing worker")
    |> maybe_add(
      is_map(worker) and is_binary(worker["worker_id"]) and String.trim(worker["worker_id"]) != "",
      "worker_snapshot.worker missing worker_id"
    )
    |> maybe_add(is_map(projection), "worker_snapshot missing projection")
    |> maybe_add(
      is_map(projection) and is_integer(projection["runtime_seq"]) and
        projection["runtime_seq"] >= 0,
      "worker_snapshot.projection runtime_seq must be >= 0"
    )
  end

  defp validate_worker_snapshot(_snapshot), do: ["codex fixture missing worker_snapshot"]

  defp validate_create_response(response) when is_map(response) do
    worker = response["worker"]
    projection = response["projection"]

    []
    |> maybe_add(is_map(worker), "worker_create_response missing worker")
    |> maybe_add(
      is_map(worker) and is_binary(worker["worker_id"]) and String.trim(worker["worker_id"]) != "",
      "worker_create_response.worker missing worker_id"
    )
    |> maybe_add(
      is_boolean(response["idempotent_replay"]),
      "worker_create_response idempotent_replay must be boolean"
    )
    |> maybe_add(is_map(projection), "worker_create_response missing projection")
  end

  defp validate_create_response(_response), do: ["codex fixture missing worker_create_response"]

  defp validate_request_response(response) when is_map(response) do
    []
    |> maybe_add(
      is_binary(response["worker_id"]) and String.trim(response["worker_id"]) != "",
      "worker_request_response missing worker_id"
    )
    |> maybe_add(
      is_binary(response["request_id"]) and String.trim(response["request_id"]) != "",
      "worker_request_response missing request_id"
    )
    |> maybe_add(is_boolean(response["ok"]), "worker_request_response ok must be boolean")
  end

  defp validate_request_response(_response), do: ["codex fixture missing worker_request_response"]

  defp validate_auth_states(states) when is_map(states) do
    authenticated = states["authenticated"]
    pending = states["device_flow_pending"]

    []
    |> maybe_add(is_map(authenticated), "auth_states missing authenticated")
    |> maybe_add(
      is_map(authenticated) and
        authenticated["hydration_status"] == "CODEX_AUTH_HYDRATION_STATUS_AUTHENTICATED",
      "auth_states.authenticated hydration_status must be AUTHENTICATED"
    )
    |> maybe_add(is_map(pending), "auth_states missing device_flow_pending")
    |> maybe_add(
      is_map(pending) and
        pending["hydration_status"] == "CODEX_AUTH_HYDRATION_STATUS_DEVICE_FLOW_PENDING",
      "auth_states.device_flow_pending hydration_status must be DEVICE_FLOW_PENDING"
    )
  end

  defp validate_auth_states(_states), do: ["codex fixture missing auth_states"]

  defp validate_notifications(events) when is_list(events) do
    errors =
      Enum.with_index(events, 1)
      |> Enum.reduce([], fn {event, index}, acc ->
        if is_map(event) do
          payload = event["payload"]
          replay = event["replay"]

          acc
          |> maybe_add(
            is_binary(event["worker_id"]) and String.trim(event["worker_id"]) != "",
            "notification event ##{index} missing worker_id"
          )
          |> maybe_add(
            is_integer(event["seq"]) and event["seq"] >= 0,
            "notification event ##{index} seq must be >= 0"
          )
          |> maybe_add(
            is_binary(event["method"]) and String.trim(event["method"]) != "",
            "notification event ##{index} missing method"
          )
          |> maybe_add(
            is_binary(event["method_text"]) and String.trim(event["method_text"]) != "",
            "notification event ##{index} missing method_text"
          )
          |> maybe_add(is_map(payload), "notification event ##{index} missing payload")
          |> maybe_add(
            is_map(payload) and is_binary(payload["source"]) and
              String.trim(payload["source"]) != "",
            "notification event ##{index} payload missing source"
          )
          |> maybe_add(
            is_map(payload) and is_binary(payload["method"]) and
              String.trim(payload["method"]) != "",
            "notification event ##{index} payload missing method"
          )
          |> maybe_add(is_map(replay), "notification event ##{index} missing replay")
          |> maybe_add(
            is_map(replay) and is_integer(replay["seq"]) and replay["seq"] == event["seq"],
            "notification event ##{index} replay.seq must equal seq"
          )
          |> maybe_add(
            is_map(replay) and is_integer(replay["resume_after"]) and replay["resume_after"] >= 0,
            "notification event ##{index} replay.resume_after must be >= 0"
          )
          |> maybe_add(
            is_map(replay) and is_integer(replay["retention_floor"]) and
              replay["retention_floor"] >= 0,
            "notification event ##{index} replay.retention_floor must be >= 0"
          )
          |> validate_notification_special_cases(index, payload)
        else
          ["notification event ##{index} must be an object" | acc]
        end
      end)

    methods =
      events
      |> Enum.filter(&is_map/1)
      |> Enum.map(& &1["method"])
      |> Enum.filter(&is_binary/1)
      |> MapSet.new()

    seqs =
      events
      |> Enum.filter(&is_map/1)
      |> Enum.map(& &1["seq"])
      |> Enum.filter(&is_integer/1)

    missing_methods =
      @required_notification_methods
      |> MapSet.new()
      |> MapSet.difference(methods)
      |> MapSet.to_list()
      |> Enum.sort()

    errors
    |> maybe_add(events != [], "codex fixture notification_events must not be empty")
    |> maybe_add(
      missing_methods == [],
      "codex fixture notification_events missing methods: #{Enum.join(missing_methods, ", ")}"
    )
    |> maybe_add(
      seqs == Enum.sort(seqs),
      "codex fixture notification seq values must be monotonic"
    )
  end

  defp validate_notifications(_events), do: ["codex fixture notification_events must be an array"]

  defp validate_notification_special_cases(errors, _index, payload) do
    case payload["method"] do
      "ios/handshake" ->
        errors
        |> maybe_add(
          is_binary(payload["handshake_id"]) and String.trim(payload["handshake_id"]) != "",
          "ios/handshake payload missing handshake_id"
        )
        |> maybe_add(
          is_binary(payload["device_id"]) and String.trim(payload["device_id"]) != "",
          "ios/handshake payload missing device_id"
        )

      "desktop/handshake_ack" ->
        errors
        |> maybe_add(
          is_binary(payload["handshake_id"]) and String.trim(payload["handshake_id"]) != "",
          "desktop/handshake_ack payload missing handshake_id"
        )
        |> maybe_add(
          is_binary(payload["desktop_session_id"]) and
            String.trim(payload["desktop_session_id"]) != "",
          "desktop/handshake_ack payload missing desktop_session_id"
        )

      "ios/user_message" ->
        params = payload["params"]

        errors
        |> maybe_add(
          is_binary(payload["message_id"]) and String.trim(payload["message_id"]) != "",
          "ios/user_message payload missing message_id"
        )
        |> maybe_add(
          is_map(params) and is_binary(params["text"]) and String.trim(params["text"]) != "",
          "ios/user_message payload missing params.text"
        )

      _other ->
        errors
    end
  end

  defp validate_stream_errors(stream_errors) when is_map(stream_errors) do
    stale = stream_errors["stale_cursor"]
    reauth = stream_errors["reauth_required"]

    []
    |> maybe_add(is_map(stale), "stream_errors missing stale_cursor")
    |> maybe_add(
      is_map(stale) and stale["code"] == "CODEX_STREAM_ERROR_CODE_STALE_CURSOR",
      "stream_errors.stale_cursor code must be STALE_CURSOR"
    )
    |> maybe_add(
      is_map(stale) and stale["full_resync_required"] == true,
      "stream_errors.stale_cursor full_resync_required must be true"
    )
    |> maybe_add(is_map(reauth), "stream_errors missing reauth_required")
    |> maybe_add(
      is_map(reauth) and reauth["code"] == "CODEX_STREAM_ERROR_CODE_REAUTH_REQUIRED",
      "stream_errors.reauth_required code must be REAUTH_REQUIRED"
    )
    |> maybe_add(
      is_map(reauth) and reauth["reauth_required"] == true,
      "stream_errors.reauth_required reauth_required must be true"
    )
  end

  defp validate_stream_errors(_stream_errors), do: ["codex fixture missing stream_errors"]

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

  defp extract_enum_body(proto_body, enum_name) do
    pattern = ~r/enum\s+#{Regex.escape(enum_name)}\s*\{(?<body>.*?)\n\}/ms

    case Regex.named_captures(pattern, proto_body) do
      %{"body" => body} -> {:ok, body}
      _ -> {:error, :enum_not_found}
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

  defp maybe_add(errors, true, _message), do: errors
  defp maybe_add(errors, false, message), do: [message | errors]
end
