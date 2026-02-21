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
  def connect(%{"token" => token}, socket, _connect_info) when is_binary(token) do
    case JwtVerifier.verify_and_claims(token, []) do
      {:ok, claims} ->
        with {:ok, session_id} <- claim_string(claims, "oa_session_id"),
             {:ok, device_id} <- claim_string(claims, "oa_device_id") do
          allowed_topics = allowed_topics(claims)

          case SessionRevocation.revoked?(session_id, device_id) do
            :active ->
              emit_auth("ok", "authorized")

              {:ok,
               socket
               |> assign(:sync_claims, claims)
               |> assign(:allowed_topics, allowed_topics)
               |> assign(:sync_session_id, session_id)
               |> assign(:sync_device_id, device_id)
               |> assign(:sync_reauth_required, false)
               |> assign(:sync_principal, principal(claims))}

            {:revoked, reason} ->
              emit_auth("error", "reauth_required")

              {:ok,
               socket
               |> assign(:sync_claims, claims)
               |> assign(:allowed_topics, allowed_topics)
               |> assign(:sync_session_id, session_id)
               |> assign(:sync_device_id, device_id)
               |> assign(:sync_reauth_required, true)
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

  defp emit_auth(status, reason_class) do
    Events.emit(@auth_event, %{count: 1}, %{
      component: "sync_socket",
      status: status,
      reason_class: reason_class
    })
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
