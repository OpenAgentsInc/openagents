defmodule OpenAgentsRuntime.Contracts.ControlPlaneAuthProtoContract do
  @moduledoc """
  Validates control-plane auth/session proto and fixture convergence.
  """

  @repo_root Path.expand("../../../../..", __DIR__)
  @proto_path Path.join(@repo_root, "proto/openagents/control/v1/auth.proto")

  @fixture_path Path.join(
                  @repo_root,
                  "docs/protocol/fixtures/control-auth-session-v1.json"
                )

  @required_messages [
    "DeviceContext",
    "AuthChallengeRequest",
    "AuthChallengeResponse",
    "AuthVerifyRequest",
    "AuthSession",
    "AuthVerifyResponse",
    "SessionRefreshRequest",
    "SessionRefreshResponse",
    "SessionRevocationRequest",
    "SessionRevocationResponse",
    "OrgMembership",
    "WorkOSIdentity",
    "SyncTokenRequest",
    "SyncTokenResponse",
    "ControlError",
    "AuthOperationResult"
  ]

  @required_auth_providers [
    "AUTH_PROVIDER_WORKOS_EMAIL_CODE"
  ]

  @required_session_statuses [
    "SESSION_STATUS_ACTIVE",
    "SESSION_STATUS_REAUTH_REQUIRED",
    "SESSION_STATUS_EXPIRED",
    "SESSION_STATUS_REVOKED"
  ]

  @required_org_roles [
    "ORG_ROLE_OWNER",
    "ORG_ROLE_ADMIN",
    "ORG_ROLE_MEMBER",
    "ORG_ROLE_VIEWER"
  ]

  @required_error_codes [
    "CONTROL_ERROR_CODE_UNAUTHORIZED",
    "CONTROL_ERROR_CODE_FORBIDDEN",
    "CONTROL_ERROR_CODE_INVALID_REQUEST",
    "CONTROL_ERROR_CODE_INVALID_SCOPE",
    "CONTROL_ERROR_CODE_RATE_LIMITED",
    "CONTROL_ERROR_CODE_REAUTH_REQUIRED",
    "CONTROL_ERROR_CODE_SESSION_EXPIRED",
    "CONTROL_ERROR_CODE_SESSION_REVOKED",
    "CONTROL_ERROR_CODE_NOT_FOUND",
    "CONTROL_ERROR_CODE_CONFLICT",
    "CONTROL_ERROR_CODE_SERVICE_UNAVAILABLE",
    "CONTROL_ERROR_CODE_INTERNAL_ERROR"
  ]

  @required_auth_result_payload_fields [
    "challenge",
    "verify",
    "refresh",
    "revoke",
    "sync_token",
    "error"
  ]

  @required_session_revoke_target_fields [
    "session_id",
    "device_id",
    "revoke_all_sessions"
  ]

  @required_fixture_error_examples [
    "sync_token_invalid_scope_error",
    "reauth_required_error",
    "session_revoked_error"
  ]

  @spec check() :: :ok | {:error, [String.t()]}
  def check do
    with {:ok, proto_body} <- read_artifact(@proto_path),
         {:ok, fixture_raw} <- read_artifact(@fixture_path),
         {:ok, fixture} <- decode_json(@fixture_path, fixture_raw) do
      errors =
        []
        |> Kernel.++(validate_required_messages(proto_body))
        |> Kernel.++(validate_enum_values(proto_body, "AuthProvider", @required_auth_providers))
        |> Kernel.++(
          validate_enum_values(proto_body, "SessionStatus", @required_session_statuses)
        )
        |> Kernel.++(validate_enum_values(proto_body, "OrgRole", @required_org_roles))
        |> Kernel.++(validate_enum_values(proto_body, "ControlErrorCode", @required_error_codes))
        |> Kernel.++(
          validate_oneof_fields(
            proto_body,
            "AuthOperationResult",
            "payload",
            @required_auth_result_payload_fields
          )
        )
        |> Kernel.++(
          validate_oneof_fields(
            proto_body,
            "SessionRevocationRequest",
            "target",
            @required_session_revoke_target_fields
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
        ["control auth proto missing message '#{message_name}'" | errors]
      end
    end)
  end

  defp validate_enum_values(proto_body, enum_name, expected_values) do
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
        ["control auth enum '#{enum_name}' missing values: #{Enum.join(missing, ", ")}"]
      end
    else
      {:error, :enum_not_found} -> ["control auth proto missing enum '#{enum_name}'"]
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
          "control auth oneof '#{message_name}.#{oneof_name}' missing fields: #{Enum.join(missing, ", ")}"
        ]
      end
    else
      {:error, :message_not_found} ->
        ["control auth proto missing message '#{message_name}'"]

      {:error, :oneof_not_found} ->
        ["control auth proto missing oneof '#{message_name}.#{oneof_name}'"]
    end
  end

  defp validate_fixture(fixture) when is_map(fixture) do
    []
    |> Kernel.++(validate_challenge_fixture(fixture["auth_challenge_success"]))
    |> Kernel.++(validate_verify_fixture(fixture["auth_verify_success"]))
    |> Kernel.++(validate_refresh_fixture(fixture["session_refresh_success"]))
    |> Kernel.++(validate_sync_token_fixture(fixture["sync_token_success"]))
    |> Kernel.++(validate_error_fixtures(fixture))
    |> Kernel.++(validate_operation_envelopes(fixture["operation_envelope_examples"]))
  end

  defp validate_fixture(_fixture), do: ["control auth fixture must be a JSON object"]

  defp validate_challenge_fixture(challenge) when is_map(challenge) do
    []
    |> maybe_add(
      is_binary(challenge["request_id"]) and String.trim(challenge["request_id"]) != "",
      "auth_challenge_success missing request_id"
    )
    |> maybe_add(
      is_binary(challenge["challenge_id"]) and String.trim(challenge["challenge_id"]) != "",
      "auth_challenge_success missing challenge_id"
    )
    |> maybe_add(
      challenge["provider"] in @required_auth_providers,
      "auth_challenge_success has invalid provider"
    )
    |> maybe_add(
      challenge["status"] == "AUTH_CHALLENGE_STATUS_CODE_SENT",
      "auth_challenge_success status must be AUTH_CHALLENGE_STATUS_CODE_SENT"
    )
    |> maybe_add(
      is_binary(challenge["email_hint"]) and String.trim(challenge["email_hint"]) != "",
      "auth_challenge_success missing email_hint"
    )
  end

  defp validate_challenge_fixture(_challenge),
    do: ["control auth fixture missing auth_challenge_success"]

  defp validate_verify_fixture(verify) when is_map(verify) do
    session = verify["session"]

    []
    |> maybe_add(
      is_binary(verify["request_id"]) and String.trim(verify["request_id"]) != "",
      "auth_verify_success missing request_id"
    )
    |> maybe_add(
      verify["token_type"] == "Bearer",
      "auth_verify_success token_type must be Bearer"
    )
    |> maybe_add(is_map(session), "auth_verify_success missing session")
    |> Kernel.++(validate_session_payload(session, "auth_verify_success.session"))
  end

  defp validate_verify_fixture(_verify), do: ["control auth fixture missing auth_verify_success"]

  defp validate_refresh_fixture(refresh) when is_map(refresh) do
    session = refresh["session"]

    []
    |> maybe_add(
      is_binary(refresh["request_id"]) and String.trim(refresh["request_id"]) != "",
      "session_refresh_success missing request_id"
    )
    |> maybe_add(
      is_binary(refresh["replaced_refresh_token_id"]) and
        String.trim(refresh["replaced_refresh_token_id"]) != "",
      "session_refresh_success missing replaced_refresh_token_id"
    )
    |> maybe_add(is_map(session), "session_refresh_success missing session")
    |> Kernel.++(validate_session_payload(session, "session_refresh_success.session"))
  end

  defp validate_refresh_fixture(_refresh),
    do: ["control auth fixture missing session_refresh_success"]

  defp validate_session_payload(session, fixture_path) when is_map(session) do
    memberships = List.wrap(session["memberships"])
    identity = session["identity"]

    []
    |> maybe_add(
      is_binary(session["session_id"]) and String.trim(session["session_id"]) != "",
      "#{fixture_path} missing session_id"
    )
    |> maybe_add(
      is_binary(session["user_id"]) and String.trim(session["user_id"]) != "",
      "#{fixture_path} missing user_id"
    )
    |> maybe_add(
      is_binary(session["device_id"]) and String.trim(session["device_id"]) != "",
      "#{fixture_path} missing device_id"
    )
    |> maybe_add(
      session["status"] == "SESSION_STATUS_ACTIVE",
      "#{fixture_path} status must be SESSION_STATUS_ACTIVE"
    )
    |> maybe_add(
      is_binary(session["access_token"]) and String.trim(session["access_token"]) != "",
      "#{fixture_path} missing access_token"
    )
    |> maybe_add(
      is_binary(session["refresh_token"]) and String.trim(session["refresh_token"]) != "",
      "#{fixture_path} missing refresh_token"
    )
    |> maybe_add(
      is_binary(session["refresh_token_id"]) and String.trim(session["refresh_token_id"]) != "",
      "#{fixture_path} missing refresh_token_id"
    )
    |> maybe_add(
      is_binary(session["active_org_id"]) and String.trim(session["active_org_id"]) != "",
      "#{fixture_path} missing active_org_id"
    )
    |> maybe_add(
      memberships != [],
      "#{fixture_path} must contain at least one org membership"
    )
    |> maybe_add(
      Enum.all?(memberships, fn membership ->
        is_map(membership) and
          is_binary(membership["org_id"]) and
          String.trim(membership["org_id"]) != "" and
          membership["role"] in @required_org_roles
      end),
      "#{fixture_path} memberships must contain org_id and valid role"
    )
    |> maybe_add(
      is_map(identity) and
        is_binary(identity["workos_user_id"]) and
        String.trim(identity["workos_user_id"]) != "" and
        is_binary(identity["email"]) and
        String.trim(identity["email"]) != "",
      "#{fixture_path} identity must include workos_user_id and email"
    )
  end

  defp validate_session_payload(_session, fixture_path), do: ["#{fixture_path} must be an object"]

  defp validate_sync_token_fixture(sync_token) when is_map(sync_token) do
    granted_scopes = List.wrap(sync_token["granted_scopes"])
    granted_topics = List.wrap(sync_token["granted_topics"])

    []
    |> maybe_add(
      is_binary(sync_token["request_id"]) and String.trim(sync_token["request_id"]) != "",
      "sync_token_success missing request_id"
    )
    |> maybe_add(
      is_binary(sync_token["session_id"]) and String.trim(sync_token["session_id"]) != "",
      "sync_token_success missing session_id"
    )
    |> maybe_add(
      is_binary(sync_token["device_id"]) and String.trim(sync_token["device_id"]) != "",
      "sync_token_success missing device_id"
    )
    |> maybe_add(
      is_binary(sync_token["token"]) and String.trim(sync_token["token"]) != "",
      "sync_token_success missing token"
    )
    |> maybe_add(
      sync_token["token_type"] == "Bearer",
      "sync_token_success token_type must be Bearer"
    )
    |> maybe_add(
      is_integer(sync_token["expires_in_seconds"]) and sync_token["expires_in_seconds"] > 0,
      "sync_token_success expires_in_seconds must be positive"
    )
    |> maybe_add(
      is_binary(sync_token["claims_version"]) and String.trim(sync_token["claims_version"]) != "",
      "sync_token_success missing claims_version"
    )
    |> maybe_add(granted_scopes != [], "sync_token_success must include granted_scopes")
    |> maybe_add(
      length(granted_scopes) == length(Enum.uniq(granted_scopes)),
      "sync_token_success granted_scopes must be unique"
    )
    |> maybe_add(
      Enum.all?(granted_topics, fn grant ->
        is_map(grant) and
          is_binary(grant["topic"]) and
          String.trim(grant["topic"]) != "" and
          is_binary(grant["required_scope"]) and
          String.trim(grant["required_scope"]) != ""
      end),
      "sync_token_success granted_topics must include topic and required_scope"
    )
  end

  defp validate_sync_token_fixture(_sync_token),
    do: ["control auth fixture missing sync_token_success"]

  defp validate_error_fixtures(fixture) do
    Enum.reduce(@required_fixture_error_examples, [], fn key, acc ->
      error_example = fixture[key]

      if is_map(error_example) do
        code = error_example["code"]
        reauth_required = error_example["reauth_required"]

        acc
        |> maybe_add(
          is_binary(error_example["request_id"]) and
            String.trim(error_example["request_id"]) != "",
          "#{key} missing request_id"
        )
        |> maybe_add(code in @required_error_codes, "#{key} has invalid control error code")
        |> maybe_add(
          is_binary(error_example["message"]) and String.trim(error_example["message"]) != "",
          "#{key} missing message"
        )
        |> maybe_add(
          is_boolean(reauth_required),
          "#{key} must include boolean reauth_required"
        )
        |> maybe_add(
          reauth_requirement_matches?(key, code, reauth_required),
          "#{key} has inconsistent reauth_required semantics"
        )
      else
        ["control auth fixture missing #{key}" | acc]
      end
    end)
  end

  defp reauth_requirement_matches?("sync_token_invalid_scope_error", _, false), do: true

  defp reauth_requirement_matches?(
         "reauth_required_error",
         "CONTROL_ERROR_CODE_REAUTH_REQUIRED",
         true
       ),
       do: true

  defp reauth_requirement_matches?(
         "session_revoked_error",
         "CONTROL_ERROR_CODE_SESSION_REVOKED",
         true
       ),
       do: true

  defp reauth_requirement_matches?(_key, _code, _reauth_required), do: false

  defp validate_operation_envelopes(examples) when is_list(examples) do
    allowed_payloads = MapSet.new(@required_auth_result_payload_fields)

    errors =
      Enum.with_index(examples, 1)
      |> Enum.reduce([], fn {example, index}, acc ->
        if is_map(example) do
          payload = example["payload"]

          acc
          |> maybe_add(
            is_binary(example["request_id"]) and String.trim(example["request_id"]) != "",
            "operation envelope ##{index} missing request_id"
          )
          |> maybe_add(
            is_binary(payload) and MapSet.member?(allowed_payloads, payload),
            "operation envelope ##{index} has invalid payload selector"
          )
          |> maybe_add(
            payload != "error" or example["error_code"] in @required_error_codes,
            "operation envelope ##{index} missing valid error_code for error payload"
          )
        else
          ["operation envelope ##{index} must be an object" | acc]
        end
      end)

    if examples == [] do
      ["control auth fixture missing operation_envelope_examples" | errors]
    else
      errors
    end
  end

  defp validate_operation_envelopes(_examples),
    do: ["control auth fixture operation_envelope_examples must be an array"]

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
