defmodule OpenAgentsRuntime.Sync.SessionRevocation do
  @moduledoc """
  Tracks revoked sync sessions/devices and triggers live websocket eviction.
  """

  alias OpenAgentsRuntime.Telemetry.Events
  alias OpenAgentsRuntimeWeb.Endpoint

  @revoked_table :openagents_runtime_sync_revoked_sessions
  @connection_table :openagents_runtime_sync_session_connections
  @revocation_event [:openagents_runtime, :sync, :socket, :revocation]
  @default_ttl_seconds 900

  @type revocation_result :: %{
          revoked_session_ids: [String.t()],
          revoked_device_ids: [String.t()],
          reason: String.t(),
          revoked_at: integer()
        }

  @spec revoke(keyword()) :: revocation_result()
  def revoke(opts \\ []) do
    revoked_session_ids =
      opts
      |> Keyword.get(:session_ids, [])
      |> normalize_ids()

    revoked_device_ids =
      opts
      |> Keyword.get(:device_ids, [])
      |> normalize_ids()

    reason = normalize_reason(Keyword.get(opts, :reason, "user_requested"))
    revoked_at = System.system_time(:second)
    expires_at = revoked_at + ttl_seconds()
    table = ensure_table()

    Enum.each(revoked_session_ids, fn session_id ->
      :ets.insert(table, {{:session, session_id}, reason, revoked_at, expires_at})

      session_id
      |> connection_pids()
      |> Enum.each(fn pid -> send(pid, {:sync_session_revoked, reason}) end)

      Endpoint.broadcast(socket_topic(session_id), "disconnect", %{"reason" => "reauth_required"})
    end)

    Enum.each(revoked_device_ids, fn device_id ->
      :ets.insert(table, {{:device, device_id}, reason, revoked_at, expires_at})
    end)

    Events.emit(@revocation_event, %{count: 1}, %{
      component: "sync_session_revocation",
      status: "ok",
      reason_class: reason,
      result: "revoked"
    })

    %{
      revoked_session_ids: revoked_session_ids,
      revoked_device_ids: revoked_device_ids,
      reason: reason,
      revoked_at: revoked_at
    }
  end

  @spec revoked?(String.t() | nil, String.t() | nil) :: :active | {:revoked, String.t()}
  def revoked?(session_id, device_id) do
    purge_expired(System.system_time(:second))

    case lookup_revocation(:session, session_id) || lookup_revocation(:device, device_id) do
      nil -> :active
      reason -> {:revoked, reason}
    end
  end

  @spec socket_topic(String.t()) :: String.t()
  def socket_topic(session_id), do: "sync_session:#{session_id}"

  @spec register_connection(String.t(), pid()) :: :ok
  def register_connection(session_id, pid)
      when is_binary(session_id) and session_id != "" and is_pid(pid) do
    ensure_connection_table()
    true = :ets.insert(@connection_table, {{session_id, pid}, true})
    :ok
  end

  def register_connection(_session_id, _pid), do: :ok

  @spec unregister_connection(String.t(), pid()) :: :ok
  def unregister_connection(session_id, pid)
      when is_binary(session_id) and session_id != "" and is_pid(pid) do
    ensure_connection_table()
    :ets.delete(@connection_table, {session_id, pid})
    :ok
  end

  def unregister_connection(_session_id, _pid), do: :ok

  @spec reset_for_tests() :: :ok
  def reset_for_tests do
    ensure_table()
    ensure_connection_table()
    :ets.delete_all_objects(@revoked_table)
    :ets.delete_all_objects(@connection_table)
    :ok
  end

  defp lookup_revocation(_scope, nil), do: nil
  defp lookup_revocation(_scope, ""), do: nil

  defp lookup_revocation(scope, id) do
    table = ensure_table()

    case :ets.lookup(table, {scope_key(scope), id}) do
      [{{_scope, ^id}, reason, _revoked_at, _expires_at}] when is_binary(reason) -> reason
      _other -> nil
    end
  end

  defp scope_key(:session), do: :session
  defp scope_key(:device), do: :device

  defp normalize_ids(ids) when is_list(ids) do
    ids
    |> Enum.filter(&is_binary/1)
    |> Enum.map(&String.trim/1)
    |> Enum.reject(&(&1 == ""))
    |> Enum.uniq()
  end

  defp normalize_ids(_ids), do: []

  defp normalize_reason(reason) when is_binary(reason) do
    normalized = reason |> String.trim() |> String.downcase()
    if normalized == "", do: "user_requested", else: normalized
  end

  defp normalize_reason(_reason), do: "user_requested"

  defp ttl_seconds do
    case Application.get_env(
           :openagents_runtime,
           :khala_sync_revocation_ttl_seconds,
           @default_ttl_seconds
         ) do
      value when is_integer(value) and value > 0 -> value
      _other -> @default_ttl_seconds
    end
  end

  defp purge_expired(now) do
    table = ensure_table()
    spec = [{{{:"$1", :"$2"}, :"$3", :"$4", :"$5"}, [{:<, :"$5", now}], [true]}]
    :ets.select_delete(table, spec)
    :ok
  end

  defp ensure_table do
    case :ets.whereis(@revoked_table) do
      :undefined ->
        :ets.new(@revoked_table, [:named_table, :public, :set, read_concurrency: true])

      table ->
        table
    end
  rescue
    ArgumentError ->
      @revoked_table
  end

  defp ensure_connection_table do
    case :ets.whereis(@connection_table) do
      :undefined ->
        :ets.new(@connection_table, [:named_table, :public, :set, read_concurrency: true])

      table ->
        table
    end
  rescue
    ArgumentError ->
      @connection_table
  end

  defp connection_pids(session_id) do
    ensure_connection_table()

    @connection_table
    |> :ets.match({{session_id, :"$1"}, true})
    |> Enum.map(fn [pid] -> pid end)
    |> Enum.filter(&is_pid/1)
  end
end
