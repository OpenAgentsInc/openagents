defmodule OpenAgentsRuntime.Runs.StreamTailer do
  @moduledoc """
  Tail run events with wakeup notifications and bounded backoff.
  """

  alias OpenAgentsRuntime.Integrations.LaravelEventMapper
  alias OpenAgentsRuntime.Runs.EventListener
  alias OpenAgentsRuntime.Runs.RunEvents

  @base_backoff_ms 50
  @max_backoff_ms 1_000

  @type stream_opt :: {:tail_timeout_ms, pos_integer()} | {:base_backoff_ms, pos_integer()}

  @spec stream(Plug.Conn.t(), String.t(), non_neg_integer(), [stream_opt()]) :: Plug.Conn.t()
  def stream(conn, run_id, cursor, opts \\ [])
      when is_binary(run_id) and is_integer(cursor) and cursor >= 0 do
    :ok = EventListener.subscribe(run_id)

    tail_timeout_ms = Keyword.get(opts, :tail_timeout_ms, default_tail_timeout_ms())
    base_backoff_ms = Keyword.get(opts, :base_backoff_ms, @base_backoff_ms)
    deadline_ms = monotonic_ms() + tail_timeout_ms

    loop(conn, run_id, cursor, deadline_ms, base_backoff_ms, base_backoff_ms)
  end

  @spec next_backoff_ms(pos_integer()) :: pos_integer()
  def next_backoff_ms(current_ms) when is_integer(current_ms) and current_ms > 0 do
    doubled = min(current_ms * 2, @max_backoff_ms)
    jitter = :rand.uniform(max(div(doubled, 4), 1)) - 1
    min(doubled + jitter, @max_backoff_ms)
  end

  defp loop(conn, run_id, cursor, deadline_ms, backoff_ms, base_backoff_ms) do
    case emit_available(conn, run_id, cursor) do
      {:closed, conn, _cursor} ->
        conn

      {:ok, conn, cursor, emitted?} ->
        if monotonic_ms() >= deadline_ms do
          conn
        else
          receive_timeout_ms = min(backoff_ms, max(deadline_ms - monotonic_ms(), 0))

          receive do
            {:run_event_notification, %{run_id: ^run_id}} ->
              loop(conn, run_id, cursor, deadline_ms, base_backoff_ms, base_backoff_ms)
          after
            receive_timeout_ms ->
              next_backoff = if emitted?, do: base_backoff_ms, else: next_backoff_ms(backoff_ms)
              loop(conn, run_id, cursor, deadline_ms, next_backoff, base_backoff_ms)
          end
        end
    end
  end

  defp emit_available(conn, run_id, cursor) do
    events = RunEvents.list_after(run_id, cursor)

    Enum.reduce_while(events, {:ok, conn, cursor, false}, fn event, state ->
      case emit_mapped_event(state, run_id, event) do
        {:ok, _conn, _cursor, _emitted?} = next_state -> {:cont, next_state}
        {:closed, _conn, _cursor} = closed_state -> {:halt, closed_state}
      end
    end)
  end

  defp emit_mapped_event({:ok, conn, _cursor, _emitted?}, run_id, event) do
    frames =
      LaravelEventMapper.map_runtime_event(run_id, event.seq, event.event_type, event.payload)

    Enum.reduce_while(frames, {:ok, conn, event.seq, true}, fn frame, {:ok, conn, cursor, _} ->
      chunk = LaravelEventMapper.to_sse_chunk(frame)

      case Plug.Conn.chunk(conn, chunk) do
        {:ok, conn} -> {:cont, {:ok, conn, cursor, true}}
        {:error, :closed} -> {:halt, {:closed, conn, cursor}}
      end
    end)
  end

  defp default_tail_timeout_ms do
    Application.get_env(:openagents_runtime, :stream_tail_timeout_ms, 1_000)
  end

  defp monotonic_ms, do: System.monotonic_time(:millisecond)
end
