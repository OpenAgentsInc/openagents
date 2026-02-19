defmodule OpenAgentsRuntimeWeb.CodexWorkerController do
  use OpenAgentsRuntimeWeb, :controller

  alias OpenAgentsRuntime.Codex.Worker
  alias OpenAgentsRuntime.Codex.WorkerStreamTailer
  alias OpenAgentsRuntime.Codex.Workers

  @spec list(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def list(conn, params) do
    with {:ok, principal} <- principal_from_headers(conn),
         {:ok, opts} <- list_opts(params),
         {:ok, workers} <- Workers.list_workers(principal, opts) do
      json(conn, %{"data" => workers})
    else
      {:error, :invalid_principal} ->
        error(conn, 401, "unauthorized", "missing or invalid principal headers")

      {:error, {:invalid_request, message}} ->
        error(conn, 400, "invalid_request", message)
    end
  end

  @spec create(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def create(conn, params) do
    with {:ok, principal} <- principal_from_headers(conn),
         {:ok, result} <- Workers.create_worker(params, principal) do
      status = if result.idempotent_replay, do: 200, else: 202

      conn
      |> put_status(status)
      |> json(%{
        "data" => %{
          "workerId" => result.worker.worker_id,
          "status" => result.worker.status,
          "latestSeq" => result.worker.latest_seq,
          "idempotentReplay" => result.idempotent_replay
        }
      })
    else
      {:error, :invalid_principal} ->
        error(conn, 401, "unauthorized", "missing or invalid principal headers")

      {:error, :forbidden} ->
        error(conn, 403, "forbidden", "worker does not belong to principal")

      {:error, %Ecto.Changeset{} = changeset} ->
        error(conn, 400, "invalid_request", inspect(changeset.errors))
    end
  end

  @spec snapshot(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def snapshot(conn, %{"worker_id" => worker_id}) do
    with {:ok, principal} <- principal_from_headers(conn),
         {:ok, snapshot} <- Workers.snapshot(worker_id, principal) do
      json(conn, %{"data" => snapshot})
    else
      {:error, :invalid_principal} ->
        error(conn, 401, "unauthorized", "missing or invalid principal headers")

      {:error, :forbidden} ->
        error(conn, 403, "forbidden", "worker does not belong to principal")

      {:error, :not_found} ->
        error(conn, 404, "not_found", "worker not found")
    end
  end

  def snapshot(conn, _params) do
    error(conn, 400, "invalid_request", "worker_id is required")
  end

  @spec request(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def request(conn, %{"worker_id" => worker_id} = params) do
    with {:ok, principal} <- principal_from_headers(conn),
         {:ok, request_payload} <- request_payload(params),
         {:ok, result} <- Workers.submit_request(worker_id, principal, request_payload) do
      json(conn, %{"data" => result})
    else
      {:error, :invalid_principal} ->
        error(conn, 401, "unauthorized", "missing or invalid principal headers")

      {:error, :invalid_request} ->
        error(conn, 400, "invalid_request", "request.method is required")

      {:error, :forbidden} ->
        error(conn, 403, "forbidden", "worker does not belong to principal")

      {:error, :not_found} ->
        error(conn, 404, "not_found", "worker not found")
    end
  end

  def request(conn, _params) do
    error(conn, 400, "invalid_request", "worker_id is required")
  end

  @spec stop(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def stop(conn, %{"worker_id" => worker_id} = params) do
    with {:ok, principal} <- principal_from_headers(conn),
         {:ok, result} <- Workers.stop_worker(worker_id, principal, reason: params["reason"]) do
      status = if result["idempotent_replay"], do: 200, else: 202

      conn
      |> put_status(status)
      |> json(%{"data" => result})
    else
      {:error, :invalid_principal} ->
        error(conn, 401, "unauthorized", "missing or invalid principal headers")

      {:error, :forbidden} ->
        error(conn, 403, "forbidden", "worker does not belong to principal")

      {:error, :not_found} ->
        error(conn, 404, "not_found", "worker not found")
    end
  end

  def stop(conn, _params) do
    error(conn, 400, "invalid_request", "worker_id is required")
  end

  @spec stream(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def stream(conn, %{"worker_id" => worker_id} = params) do
    with {:ok, principal} <- principal_from_headers(conn),
         {:ok, _snapshot} <- Workers.snapshot(worker_id, principal),
         {:ok, cursor} <- resolve_cursor(conn, params, worker_id),
         {:ok, tail_timeout_ms} <- parse_tail_timeout(params),
         :ok <- validate_cursor_window(worker_id, cursor) do
      conn =
        conn
        |> put_resp_content_type("text/event-stream")
        |> put_resp_header("cache-control", "no-cache")
        |> put_resp_header("x-accel-buffering", "no")
        |> send_chunked(200)

      WorkerStreamTailer.stream(conn, worker_id, cursor, tail_timeout_ms: tail_timeout_ms)
    else
      {:error, :invalid_principal} ->
        error(conn, 401, "unauthorized", "missing or invalid principal headers")

      {:error, :forbidden} ->
        error(conn, 403, "forbidden", "worker does not belong to principal")

      {:error, :not_found} ->
        error(conn, 404, "not_found", "worker not found")

      {:error, :invalid_cursor} ->
        error(conn, 400, "invalid_request", "cursor must be an integer >= 0")

      {:error, :cursor_mismatch} ->
        error(conn, 400, "invalid_request", "cursor query and Last-Event-ID must match")

      {:error, :stale_cursor} ->
        error(conn, 410, "stale_cursor", "cursor is older than retention floor")

      {:error, :invalid_tail_timeout} ->
        error(conn, 400, "invalid_request", "tail_ms must be a positive integer")
    end
  end

  def stream(conn, _params) do
    error(conn, 400, "invalid_request", "worker_id is required")
  end

  defp principal_from_headers(conn) do
    principal =
      %{}
      |> maybe_put_user_id(get_req_header(conn, "x-oa-user-id"))
      |> maybe_put_guest_scope(get_req_header(conn, "x-oa-guest-scope"))

    Workers.normalize_principal(principal)
  end

  defp maybe_put_user_id(principal, [user_id]) do
    case Integer.parse(user_id) do
      {parsed, ""} when parsed > 0 -> Map.put(principal, :user_id, parsed)
      _ -> principal
    end
  end

  defp maybe_put_user_id(principal, _), do: principal

  defp maybe_put_guest_scope(principal, [guest_scope]) when byte_size(guest_scope) > 0 do
    Map.put(principal, :guest_scope, guest_scope)
  end

  defp maybe_put_guest_scope(principal, _), do: principal

  defp request_payload(%{"request" => request}) when is_map(request), do: {:ok, request}

  defp request_payload(%{"method" => _method} = params), do: {:ok, params}

  defp request_payload(_), do: {:error, :invalid_request}

  defp resolve_cursor(conn, params, worker_id) do
    query_cursor = params |> Map.get("cursor") |> parse_cursor()
    header_cursor = conn |> get_req_header("last-event-id") |> List.first() |> parse_cursor()

    with :ok <- validate_cursor_pair(query_cursor, header_cursor),
         {:ok, cursor} <- pick_cursor(query_cursor, header_cursor, worker_id) do
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
  defp validate_cursor_pair({:ok, _value}, :missing), do: :ok
  defp validate_cursor_pair(:missing, {:ok, _value}), do: :ok
  defp validate_cursor_pair({:ok, value}, {:ok, value}), do: :ok
  defp validate_cursor_pair({:ok, _}, {:ok, _}), do: {:error, :cursor_mismatch}

  defp pick_cursor(:missing, :missing, worker_id), do: {:ok, Workers.latest_seq(worker_id)}
  defp pick_cursor({:ok, value}, :missing, _worker_id), do: {:ok, value}
  defp pick_cursor(:missing, {:ok, value}, _worker_id), do: {:ok, value}
  defp pick_cursor({:ok, value}, {:ok, value}, _worker_id), do: {:ok, value}
  defp pick_cursor(_, _, _), do: {:error, :cursor_mismatch}

  defp validate_cursor_window(worker_id, cursor) do
    oldest_seq = Workers.oldest_seq(worker_id)
    retention_floor = max(oldest_seq - 1, 0)

    if cursor < retention_floor do
      {:error, :stale_cursor}
    else
      :ok
    end
  end

  defp parse_tail_timeout(params) do
    case Map.get(params, "tail_ms") do
      nil ->
        {:ok, Application.get_env(:openagents_runtime, :stream_tail_timeout_ms, 1_000)}

      value ->
        case Integer.parse(value) do
          {parsed, ""} when parsed > 0 -> {:ok, parsed}
          _ -> {:error, :invalid_tail_timeout}
        end
    end
  end

  defp list_opts(params) do
    with {:ok, limit} <- parse_limit(params),
         {:ok, status} <- parse_status(params),
         {:ok, workspace_ref} <- parse_workspace_ref(params) do
      opts =
        []
        |> Keyword.put(:limit, limit)
        |> maybe_put_opt(:status, status)
        |> maybe_put_opt(:workspace_ref, workspace_ref)

      {:ok, opts}
    end
  end

  defp parse_limit(params) do
    case Map.get(params, "limit") do
      nil ->
        {:ok, 50}

      value when is_binary(value) ->
        case Integer.parse(value) do
          {parsed, ""} when parsed >= 1 and parsed <= 200 -> {:ok, parsed}
          _ -> {:error, {:invalid_request, "limit must be an integer between 1 and 200"}}
        end

      _ ->
        {:error, {:invalid_request, "limit must be an integer between 1 and 200"}}
    end
  end

  defp parse_status(params) do
    case trim_string(Map.get(params, "status")) do
      nil ->
        {:ok, nil}

      status ->
        if status in Worker.statuses() do
          {:ok, status}
        else
          {:error,
           {:invalid_request, "status must be one of: #{Enum.join(Worker.statuses(), ", ")}"}}
        end
    end
  end

  defp parse_workspace_ref(params) do
    workspace_ref = trim_string(Map.get(params, "workspace_ref"))

    case workspace_ref do
      nil ->
        {:ok, nil}

      value when byte_size(value) <= 255 ->
        {:ok, value}

      _ ->
        {:error, {:invalid_request, "workspace_ref must be <= 255 chars"}}
    end
  end

  defp trim_string(value) when is_binary(value) do
    value
    |> String.trim()
    |> case do
      "" -> nil
      trimmed -> trimmed
    end
  end

  defp trim_string(_), do: nil

  defp maybe_put_opt(opts, _key, nil), do: opts
  defp maybe_put_opt(opts, key, value), do: Keyword.put(opts, key, value)

  defp error(conn, status, code, message) do
    conn
    |> put_status(status)
    |> json(%{"error" => %{"code" => code, "message" => message}})
  end
end
