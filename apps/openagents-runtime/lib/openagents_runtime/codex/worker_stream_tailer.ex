defmodule OpenAgentsRuntime.Codex.WorkerStreamTailer do
  @moduledoc """
  SSE tailer for Codex worker event streams.
  """

  alias OpenAgentsRuntime.Codex.Workers

  @base_backoff_ms 50
  @max_backoff_ms 1_000

  @spec stream(Plug.Conn.t(), String.t(), non_neg_integer(), keyword()) :: Plug.Conn.t()
  def stream(conn, worker_id, cursor, opts \\ [])
      when is_binary(worker_id) and is_integer(cursor) and cursor >= 0 do
    :ok = Workers.subscribe(worker_id)

    tail_timeout_ms = Keyword.get(opts, :tail_timeout_ms, default_tail_timeout_ms())
    base_backoff_ms = Keyword.get(opts, :base_backoff_ms, @base_backoff_ms)
    deadline_ms = monotonic_ms() + tail_timeout_ms

    {conn, _cursor} = loop(conn, worker_id, cursor, deadline_ms, base_backoff_ms, base_backoff_ms)
    conn
  end

  defp loop(conn, worker_id, cursor, deadline_ms, backoff_ms, base_backoff_ms) do
    case emit_available(conn, worker_id, cursor) do
      {:closed, conn, cursor, _emitted?} ->
        {conn, cursor}

      {:ok, conn, cursor, emitted?} ->
        if monotonic_ms() >= deadline_ms do
          {conn, cursor}
        else
          receive_timeout_ms = min(backoff_ms, max(deadline_ms - monotonic_ms(), 0))

          receive do
            {:codex_worker_event_notification, %{worker_id: ^worker_id}} ->
              loop(conn, worker_id, cursor, deadline_ms, base_backoff_ms, base_backoff_ms)
          after
            receive_timeout_ms ->
              next_backoff = if emitted?, do: base_backoff_ms, else: next_backoff_ms(backoff_ms)
              loop(conn, worker_id, cursor, deadline_ms, next_backoff, base_backoff_ms)
          end
        end
    end
  end

  defp emit_available(conn, worker_id, cursor) do
    events = Workers.list_after(worker_id, cursor)

    Enum.reduce_while(events, {:ok, conn, cursor, false}, fn event,
                                                             {:ok, conn, _cursor, _emitted?} ->
      chunk = encode_event(event)

      case Plug.Conn.chunk(conn, chunk) do
        {:ok, conn} -> {:cont, {:ok, conn, event.seq, true}}
        {:error, :closed} -> {:halt, {:closed, conn, event.seq, true}}
      end
    end)
  end

  defp encode_event(event) do
    payload = %{
      "workerId" => event.worker_id,
      "seq" => event.seq,
      "eventType" => event.event_type,
      "payload" => event.payload,
      "occurredAt" => DateTime.to_iso8601(event.inserted_at)
    }

    [
      "id: #{event.seq}",
      "event: codex.worker.event",
      "data: #{Jason.encode!(payload)}",
      ""
    ]
    |> Enum.join("\n")
  end

  defp next_backoff_ms(current_ms) when is_integer(current_ms) and current_ms > 0 do
    doubled = min(current_ms * 2, @max_backoff_ms)
    jitter = :rand.uniform(max(div(doubled, 4), 1)) - 1
    min(doubled + jitter, @max_backoff_ms)
  end

  defp default_tail_timeout_ms do
    Application.get_env(:openagents_runtime, :stream_tail_timeout_ms, 1_000)
  end

  defp monotonic_ms, do: System.monotonic_time(:millisecond)
end
