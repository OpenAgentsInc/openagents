defmodule OpenAgentsRuntimeWeb.RunController do
  use OpenAgentsRuntimeWeb, :controller

  alias OpenAgentsRuntime.Runs.OwnershipGuard
  alias OpenAgentsRuntime.Telemetry.Tracing

  @spec snapshot(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def snapshot(conn, %{"run_id" => run_id, "thread_id" => thread_id}) do
    Tracing.with_phase_span(:persist, %{run_id: run_id, thread_id: thread_id}, fn ->
      principal = principal_from_headers(conn)

      with {:ok, principal} <- OwnershipGuard.normalize_principal(principal),
           :ok <- OwnershipGuard.authorize(run_id, thread_id, principal) do
        json(conn, %{
          "runId" => run_id,
          "threadId" => thread_id,
          "status" => "unknown",
          "latestSeq" => 0,
          "updatedAt" => DateTime.utc_now() |> DateTime.truncate(:second) |> DateTime.to_iso8601()
        })
      else
        {:error, :invalid_principal} ->
          error(conn, 401, "unauthorized", "missing or invalid principal headers")

        {:error, :not_found} ->
          error(conn, 404, "not_found", "run/thread ownership record not found")

        {:error, :forbidden} ->
          error(conn, 403, "forbidden", "run/thread does not belong to principal")
      end
    end)
  end

  def snapshot(conn, _params) do
    error(conn, 400, "invalid_request", "thread_id is required")
  end

  defp principal_from_headers(conn) do
    %{}
    |> maybe_put_user_id(get_req_header(conn, "x-oa-user-id"))
    |> maybe_put_guest_scope(get_req_header(conn, "x-oa-guest-scope"))
  end

  defp maybe_put_user_id(principal, [user_id]) do
    case Integer.parse(user_id) do
      {parsed, ""} -> Map.put(principal, :user_id, parsed)
      _ -> principal
    end
  end

  defp maybe_put_user_id(principal, _), do: principal

  defp maybe_put_guest_scope(principal, [guest_scope]) when byte_size(guest_scope) > 0 do
    Map.put(principal, :guest_scope, guest_scope)
  end

  defp maybe_put_guest_scope(principal, _), do: principal

  defp error(conn, status, code, message) do
    conn
    |> put_status(status)
    |> json(%{"error" => %{"code" => code, "message" => message}})
  end
end
