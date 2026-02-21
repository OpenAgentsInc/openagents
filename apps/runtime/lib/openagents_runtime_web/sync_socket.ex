defmodule OpenAgentsRuntimeWeb.SyncSocket do
  @moduledoc """
  WebSocket auth boundary for Khala sync channels.
  """

  use Phoenix.Socket

  alias OpenAgentsRuntime.Sync.SessionRevocation
  alias OpenAgentsRuntime.Sync.JwtVerifier
  alias OpenAgentsRuntime.Telemetry.Events

  @known_topics MapSet.new([
                  "runtime.run_summaries",
                  "runtime.codex_worker_summaries",
                  "runtime.codex_worker_events",
                  "runtime.notifications"
                ])
  @auth_event [:openagents_runtime, :sync, :socket, :auth]

  channel "sync:v1", OpenAgentsRuntimeWeb.SyncChannel

  @impl true
  def connect(%{"token" => token} = params, socket, _connect_info) when is_binary(token) do
    case JwtVerifier.verify_and_claims(token, []) do
      {:ok, claims} ->
        with {:ok, session_id} <- claim_string(claims, "oa_session_id"),
             {:ok, device_id} <- claim_string(claims, "oa_device_id") do
          allowed_topics = allowed_topics(claims)
          client_name = resolve_client_name(params, claims)
          client_build_id = resolve_client_build_id(params, claims)

          case SessionRevocation.revoked?(session_id, device_id) do
            :active ->
              case compatibility_failure(params, claims) do
                nil ->
                  emit_auth("ok", "authorized")

                  {:ok,
                   socket
                   |> assign(:sync_claims, claims)
                   |> assign(:allowed_topics, allowed_topics)
                   |> assign(:sync_session_id, session_id)
                   |> assign(:sync_device_id, device_id)
                   |> assign(:sync_reauth_required, false)
                   |> assign(:sync_compatibility_failure, nil)
                   |> assign(:sync_principal, principal(claims))}

                failure ->
                  emit_auth(
                    "error",
                    failure["code"],
                    %{
                      surface: "khala_websocket",
                      client: client_name,
                      client_build_id: client_build_id
                    }
                  )

                  {:ok,
                   socket
                   |> assign(:sync_claims, claims)
                   |> assign(:allowed_topics, allowed_topics)
                   |> assign(:sync_session_id, session_id)
                   |> assign(:sync_device_id, device_id)
                   |> assign(:sync_reauth_required, false)
                   |> assign(:sync_compatibility_failure, failure)
                   |> assign(:sync_principal, principal(claims))}
              end

            {:revoked, reason} ->
              emit_auth("error", "reauth_required")

              {:ok,
               socket
               |> assign(:sync_claims, claims)
               |> assign(:allowed_topics, allowed_topics)
               |> assign(:sync_session_id, session_id)
               |> assign(:sync_device_id, device_id)
               |> assign(:sync_reauth_required, true)
               |> assign(:sync_compatibility_failure, nil)
               |> assign(:sync_reauth_reason, reason)
               |> assign(:sync_principal, principal(claims))}
          end
        else
          _error ->
            emit_auth("error", "invalid_claims")
            :error
        end

      {:error, reason} ->
        emit_auth("error", auth_reason(reason))
        :error
    end
  end

  def connect(_params, _socket, _connect_info) do
    emit_auth("error", "missing_token")
    :error
  end

  @impl true
  def id(socket) do
    case socket.assigns[:sync_session_id] do
      session_id when is_binary(session_id) and session_id != "" ->
        SessionRevocation.socket_topic(session_id)

      _other ->
        nil
    end
  end

  defp allowed_topics(claims) when is_map(claims) do
    claims
    |> Map.get("oa_sync_scopes", [])
    |> normalize_scopes()
    |> Enum.filter(&MapSet.member?(@known_topics, &1))
    |> Enum.uniq()
  end

  defp normalize_scopes(scopes) when is_list(scopes) do
    scopes
    |> Enum.filter(&is_binary/1)
    |> Enum.map(&String.trim/1)
    |> Enum.reject(&(&1 == ""))
  end

  defp normalize_scopes(scopes) when is_binary(scopes) do
    scopes
    |> String.split(",")
    |> normalize_scopes()
  end

  defp normalize_scopes(_scopes), do: []

  defp principal(claims) do
    %{
      sub: Map.get(claims, "sub"),
      oa_org_id: Map.get(claims, "oa_org_id"),
      oa_session_id: Map.get(claims, "oa_session_id"),
      oa_device_id: Map.get(claims, "oa_device_id")
    }
  end

  defp compatibility_failure(params, claims) do
    config = Application.get_env(:openagents_runtime, :khala_sync_auth, [])

    if Keyword.get(config, :compat_enforced, false) do
      evaluate_compatibility(params, claims, config)
    else
      nil
    end
  end

  defp evaluate_compatibility(params, claims, config) do
    protocol_version =
      normalize_string(Keyword.get(config, :compat_protocol_version, "khala.ws.v1"))

    min_client_build_id =
      normalize_string(Keyword.get(config, :compat_min_client_build_id, "00000000T000000Z"))

    max_client_build_id =
      normalize_optional_string(Keyword.get(config, :compat_max_client_build_id, nil))

    min_schema_version =
      normalize_positive_integer(Keyword.get(config, :compat_min_schema_version, 1), 1)

    max_schema_version =
      normalize_positive_integer(
        Keyword.get(config, :compat_max_schema_version, 1),
        min_schema_version
      )

    client_build_id = resolve_client_build_id(params, claims)
    client_protocol_version = resolve_protocol_version(params, claims)
    client_schema_version = resolve_schema_version(params, claims)

    cond do
      client_build_id == "" ->
        compatibility_error(
          "invalid_client_build",
          "client_build_id is required",
          protocol_version,
          min_client_build_id,
          max_client_build_id,
          min_schema_version,
          max_schema_version
        )

      client_protocol_version != protocol_version ->
        compatibility_error(
          "unsupported_protocol_version",
          "protocol version '#{client_protocol_version}' is unsupported; expected '#{protocol_version}'",
          protocol_version,
          min_client_build_id,
          max_client_build_id,
          min_schema_version,
          max_schema_version
        )

      client_schema_version < min_schema_version or client_schema_version > max_schema_version ->
        compatibility_error(
          "unsupported_schema_version",
          "schema_version #{client_schema_version} is outside supported range #{min_schema_version}..#{max_schema_version}",
          protocol_version,
          min_client_build_id,
          max_client_build_id,
          min_schema_version,
          max_schema_version
        )

      client_build_id < min_client_build_id ->
        compatibility_error(
          "upgrade_required",
          "client build '#{client_build_id}' is older than minimum supported '#{min_client_build_id}'",
          protocol_version,
          min_client_build_id,
          max_client_build_id,
          min_schema_version,
          max_schema_version
        )

      is_binary(max_client_build_id) and max_client_build_id != "" and
          client_build_id > max_client_build_id ->
        compatibility_error(
          "unsupported_client_build",
          "client build '#{client_build_id}' is newer than supported maximum '#{max_client_build_id}'",
          protocol_version,
          min_client_build_id,
          max_client_build_id,
          min_schema_version,
          max_schema_version
        )

      true ->
        nil
    end
  end

  defp compatibility_error(
         code,
         message,
         protocol_version,
         min_client_build_id,
         max_client_build_id,
         min_schema_version,
         max_schema_version
       ) do
    %{
      "code" => code,
      "message" => message,
      "retryable" => false,
      "upgrade_required" => true,
      "surface" => "khala_websocket",
      "min_client_build_id" => min_client_build_id,
      "max_client_build_id" => max_client_build_id,
      "min_schema_version" => min_schema_version,
      "max_schema_version" => max_schema_version,
      "protocol_version" => protocol_version
    }
  end

  defp resolve_client_name(params, claims) do
    normalize_string(Map.get(params, "client") || Map.get(claims, "oa_client") || "unknown")
  end

  defp resolve_client_build_id(params, claims) do
    normalize_string(
      Map.get(params, "client_build_id") || Map.get(claims, "oa_client_build_id") || ""
    )
  end

  defp resolve_protocol_version(params, claims) do
    normalize_string(
      Map.get(params, "protocol_version") || Map.get(claims, "oa_protocol_version") || ""
    )
  end

  defp resolve_schema_version(params, claims) do
    value = Map.get(params, "schema_version") || Map.get(claims, "oa_schema_version") || 0

    case value do
      parsed when is_integer(parsed) ->
        max(parsed, 0)

      parsed when is_binary(parsed) ->
        case Integer.parse(parsed) do
          {integer, ""} -> max(integer, 0)
          _other -> 0
        end

      _other ->
        0
    end
  end

  defp normalize_string(value) when is_binary(value), do: String.trim(value)
  defp normalize_string(_value), do: ""

  defp normalize_optional_string(value) when is_binary(value) do
    trimmed = String.trim(value)
    if trimmed == "", do: nil, else: trimmed
  end

  defp normalize_optional_string(_value), do: nil

  defp normalize_positive_integer(value, _fallback) when is_integer(value) and value > 0,
    do: value

  defp normalize_positive_integer(value, fallback) when is_binary(value) do
    case Integer.parse(String.trim(value)) do
      {integer, ""} when integer > 0 -> integer
      _other -> fallback
    end
  end

  defp normalize_positive_integer(_value, fallback), do: fallback

  defp emit_auth(status, reason_class, metadata \\ %{}) do
    base_metadata = %{
      component: "sync_socket",
      status: status,
      reason_class: reason_class
    }

    Events.emit(@auth_event, %{count: 1}, Map.merge(base_metadata, metadata))
  end

  defp auth_reason({:claim_mismatch, _claim}), do: "claim_mismatch"
  defp auth_reason(reason) when is_atom(reason), do: Atom.to_string(reason)
  defp auth_reason(_reason), do: "invalid_token"

  defp claim_string(claims, key) do
    case Map.get(claims, key) do
      value when is_binary(value) and value != "" -> {:ok, value}
      _other -> :error
    end
  end
end
