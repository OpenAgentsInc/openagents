defmodule OpenAgentsRuntimeWeb.SyncSocket do
  @moduledoc """
  WebSocket auth boundary for Khala sync channels.
  """

  use Phoenix.Socket

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
        allowed_topics = allowed_topics(claims)
        emit_auth("ok", "authorized")

        {:ok,
         socket
         |> assign(:sync_claims, claims)
         |> assign(:allowed_topics, allowed_topics)
         |> assign(:sync_principal, principal(claims))}

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
  def id(_socket), do: nil

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
      oa_org_id: Map.get(claims, "oa_org_id")
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
end
