defmodule OpenAgentsRuntimeWeb.SyncSessionController do
  use OpenAgentsRuntimeWeb, :controller

  alias OpenAgentsRuntime.Sync.SessionRevocation

  @spec revoke(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def revoke(conn, params) do
    session_ids = normalize_ids(Map.get(params, "session_ids"))
    device_ids = normalize_ids(Map.get(params, "device_ids"))

    if session_ids == [] and device_ids == [] do
      error(
        conn,
        400,
        "invalid_request",
        "session_ids or device_ids must include at least one identifier"
      )
    else
      result =
        SessionRevocation.revoke(
          session_ids: session_ids,
          device_ids: device_ids,
          reason: Map.get(params, "reason", "user_requested")
        )

      json(conn, %{"data" => result})
    end
  end

  defp normalize_ids(ids) when is_list(ids) do
    ids
    |> Enum.filter(&is_binary/1)
    |> Enum.map(&String.trim/1)
    |> Enum.reject(&(&1 == ""))
    |> Enum.uniq()
  end

  defp normalize_ids(_ids), do: []

  defp error(conn, status, code, message) do
    conn
    |> put_status(status)
    |> json(%{"error" => %{"code" => code, "message" => message}})
  end
end
