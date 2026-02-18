defmodule OpenAgentsRuntimeWeb.RunController do
  use OpenAgentsRuntimeWeb, :controller

  alias OpenAgentsRuntime.Runs.Frames
  alias OpenAgentsRuntime.Runs.OwnershipGuard
  alias OpenAgentsRuntime.Runs.RunEvents
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

  @spec append_frame(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def append_frame(conn, %{"run_id" => run_id, "thread_id" => thread_id} = params) do
    Tracing.with_phase_span(:ingest, %{run_id: run_id, thread_id: thread_id}, fn ->
      principal = principal_from_headers(conn)

      with {:ok, principal} <- OwnershipGuard.normalize_principal(principal),
           :ok <- OwnershipGuard.authorize(run_id, thread_id, principal),
           {:ok, result} <- Frames.append_frame(run_id, params) do
        status = if result.idempotent_replay, do: 200, else: 202

        conn
        |> put_status(status)
        |> json(%{
          "runId" => run_id,
          "frameId" => result.frame.frame_id,
          "status" => "accepted",
          "idempotentReplay" => result.idempotent_replay
        })
      else
        {:error, :invalid_principal} ->
          error(conn, 401, "unauthorized", "missing or invalid principal headers")

        {:error, :not_found} ->
          error(conn, 404, "not_found", "run/thread ownership record not found")

        {:error, :forbidden} ->
          error(conn, 403, "forbidden", "run/thread does not belong to principal")

        {:error, :run_not_found} ->
          error(conn, 404, "not_found", "run not found")

        {:error, :idempotency_conflict} ->
          error(conn, 409, "conflict", "frame_id payload mismatch for existing frame")

        {:error, %Ecto.Changeset{} = changeset} ->
          error(conn, 400, "invalid_request", inspect(changeset.errors))
      end
    end)
  end

  def append_frame(conn, _params) do
    error(conn, 400, "invalid_request", "thread_id is required")
  end

  @spec stream(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def stream(conn, %{"run_id" => run_id, "thread_id" => thread_id} = params) do
    Tracing.with_phase_span(:stream, %{run_id: run_id, thread_id: thread_id}, fn ->
      principal = principal_from_headers(conn)

      with {:ok, principal} <- OwnershipGuard.normalize_principal(principal),
           :ok <- OwnershipGuard.authorize(run_id, thread_id, principal),
           {:ok, cursor} <- resolve_cursor(conn, params, run_id),
           :ok <- validate_cursor_window(run_id, cursor) do
        events = RunEvents.list_after(run_id, cursor)
        send_event_stream(conn, run_id, events)
      else
        {:error, :invalid_principal} ->
          error(conn, 401, "unauthorized", "missing or invalid principal headers")

        {:error, :not_found} ->
          error(conn, 404, "not_found", "run/thread ownership record not found")

        {:error, :forbidden} ->
          error(conn, 403, "forbidden", "run/thread does not belong to principal")

        {:error, :invalid_cursor} ->
          error(conn, 400, "invalid_request", "cursor must be an integer >= 0")

        {:error, :cursor_mismatch} ->
          error(conn, 400, "invalid_request", "cursor query and Last-Event-ID must match")

        {:error, :stale_cursor} ->
          error(conn, 410, "stale_cursor", "cursor is older than retention floor")
      end
    end)
  end

  def stream(conn, _params), do: error(conn, 400, "invalid_request", "thread_id is required")

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

  defp resolve_cursor(conn, params, run_id) do
    query_cursor = params |> Map.get("cursor") |> parse_cursor()
    header_cursor = conn |> get_req_header("last-event-id") |> List.first() |> parse_cursor()

    with :ok <- validate_cursor_pair(query_cursor, header_cursor),
         {:ok, cursor} <- pick_cursor(query_cursor, header_cursor, run_id) do
      {:ok, cursor}
    end
  end

  defp parse_cursor(nil), do: :missing

  defp parse_cursor(cursor) when is_binary(cursor) do
    case Integer.parse(cursor) do
      {value, ""} when value >= 0 -> {:ok, value}
      _ -> {:error, :invalid_cursor}
    end
  end

  defp parse_cursor(_), do: {:error, :invalid_cursor}

  defp validate_cursor_pair({:error, :invalid_cursor}, _), do: {:error, :invalid_cursor}
  defp validate_cursor_pair(_, {:error, :invalid_cursor}), do: {:error, :invalid_cursor}
  defp validate_cursor_pair(:missing, :missing), do: :ok
  defp validate_cursor_pair({:ok, value}, :missing), do: {:ok, value}
  defp validate_cursor_pair(:missing, {:ok, value}), do: {:ok, value}
  defp validate_cursor_pair({:ok, value}, {:ok, value}), do: {:ok, value}
  defp validate_cursor_pair({:ok, _}, {:ok, _}), do: {:error, :cursor_mismatch}

  defp pick_cursor(:missing, :missing, run_id), do: {:ok, RunEvents.latest_seq(run_id)}
  defp pick_cursor({:ok, value}, :missing, _run_id), do: {:ok, value}
  defp pick_cursor(:missing, {:ok, value}, _run_id), do: {:ok, value}
  defp pick_cursor({:ok, value}, {:ok, value}, _run_id), do: {:ok, value}
  defp pick_cursor(_, _, _), do: {:error, :cursor_mismatch}

  defp validate_cursor_window(run_id, cursor) do
    oldest_seq = RunEvents.oldest_seq(run_id)
    retention_floor = max(oldest_seq - 1, 0)

    if cursor < retention_floor do
      {:error, :stale_cursor}
    else
      :ok
    end
  end

  defp send_event_stream(conn, run_id, events) do
    conn =
      conn
      |> put_resp_content_type("text/event-stream")
      |> put_resp_header("cache-control", "no-cache")
      |> put_resp_header("x-accel-buffering", "no")
      |> send_chunked(200)

    Enum.reduce_while(events, conn, fn event, conn ->
      payload =
        Jason.encode!(%{
          "runId" => run_id,
          "seq" => event.seq,
          "type" => event.event_type,
          "payload" => event.payload
        })

      chunk = "event: run.event\\nid: #{event.seq}\\ndata: #{payload}\\n\\n"

      case chunk(conn, chunk) do
        {:ok, conn} -> {:cont, conn}
        {:error, :closed} -> {:halt, conn}
      end
    end)
  end

  defp error(conn, status, code, message) do
    conn
    |> put_status(status)
    |> json(%{"error" => %{"code" => code, "message" => message}})
  end
end
