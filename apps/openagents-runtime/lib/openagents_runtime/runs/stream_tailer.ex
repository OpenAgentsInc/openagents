defmodule OpenAgentsRuntime.Runs.StreamTailer do
  @moduledoc """
  Tail run events with wakeup notifications and bounded backoff.
  """

  alias OpenAgentsRuntime.Integrations.LaravelEventMapper
  alias OpenAgentsRuntime.Runs.EventListener
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Telemetry.Events

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
    started_at = System.monotonic_time()

    summary = %{
      emitted_events: 0,
      emitted_chunks: 0,
      wakeups: 0,
      polls: 0,
      final_cursor: cursor,
      outcome: "tail_timeout"
    }

    {conn, summary} =
      loop(conn, run_id, cursor, deadline_ms, base_backoff_ms, base_backoff_ms, summary)

    Events.emit(
      [:openagents_runtime, :stream, :session],
      %{
        count: 1,
        duration_ms: elapsed_ms(started_at),
        emitted_events: summary.emitted_events,
        emitted_chunks: summary.emitted_chunks,
        wakeups: summary.wakeups,
        polls: summary.polls
      },
      %{
        run_id: run_id,
        initial_cursor: cursor,
        final_cursor: summary.final_cursor,
        outcome: summary.outcome
      }
    )

    conn
  end

  @spec next_backoff_ms(pos_integer()) :: pos_integer()
  def next_backoff_ms(current_ms) when is_integer(current_ms) and current_ms > 0 do
    doubled = min(current_ms * 2, @max_backoff_ms)
    jitter = :rand.uniform(max(div(doubled, 4), 1)) - 1
    min(doubled + jitter, @max_backoff_ms)
  end

  defp loop(conn, run_id, cursor, deadline_ms, backoff_ms, base_backoff_ms, summary) do
    case emit_available(conn, run_id, cursor) do
      {:closed, conn, cursor, emitted_events, emitted_chunks} ->
        summary =
          summary
          |> increment_stream_totals(emitted_events, emitted_chunks)
          |> Map.put(:final_cursor, cursor)
          |> Map.put(:outcome, "client_closed")

        {conn, summary}

      {:ok, conn, cursor, emitted?, emitted_events, emitted_chunks} ->
        summary =
          summary
          |> increment_stream_totals(emitted_events, emitted_chunks)
          |> Map.put(:final_cursor, cursor)

        if monotonic_ms() >= deadline_ms do
          {conn, Map.put(summary, :outcome, "tail_timeout")}
        else
          receive_timeout_ms = min(backoff_ms, max(deadline_ms - monotonic_ms(), 0))

          receive do
            {:run_event_notification, %{run_id: ^run_id}} ->
              next_summary = Map.update!(summary, :wakeups, &(&1 + 1))

              loop(
                conn,
                run_id,
                cursor,
                deadline_ms,
                base_backoff_ms,
                base_backoff_ms,
                next_summary
              )
          after
            receive_timeout_ms ->
              next_summary = Map.update!(summary, :polls, &(&1 + 1))
              next_backoff = if emitted?, do: base_backoff_ms, else: next_backoff_ms(backoff_ms)

              loop(
                conn,
                run_id,
                cursor,
                deadline_ms,
                next_backoff,
                base_backoff_ms,
                next_summary
              )
          end
        end
    end
  end

  defp emit_available(conn, run_id, cursor) do
    events = RunEvents.list_after(run_id, cursor)

    Enum.reduce_while(events, {:ok, conn, cursor, false, 0, 0}, fn event,
                                                                   {:ok, conn, _cursor, _emitted?,
                                                                    event_count, chunk_count} ->
      case emit_mapped_event(conn, run_id, event) do
        {:ok, conn, emitted_chunks} ->
          {:cont, {:ok, conn, event.seq, true, event_count + 1, chunk_count + emitted_chunks}}

        {:closed, conn, emitted_chunks} ->
          {:halt, {:closed, conn, event.seq, event_count + 1, chunk_count + emitted_chunks}}
      end
    end)
  end

  defp emit_mapped_event(conn, run_id, event) do
    frames =
      LaravelEventMapper.map_runtime_event(run_id, event.seq, event.event_type, event.payload)

    {result, emitted_chunks} =
      Enum.reduce_while(frames, {{:ok, conn}, 0}, fn frame, {{:ok, conn}, emitted_chunks} ->
        chunk = LaravelEventMapper.to_sse_chunk(frame)

        case Plug.Conn.chunk(conn, chunk) do
          {:ok, conn} -> {:cont, {{:ok, conn}, emitted_chunks + 1}}
          {:error, :closed} -> {:halt, {{:closed, conn}, emitted_chunks}}
        end
      end)

    outcome =
      case result do
        {:ok, _conn} -> "ok"
        {:closed, _conn} -> "closed"
      end

    Events.emit(
      [:openagents_runtime, :stream, :emit],
      %{count: 1, frames: emitted_chunks},
      %{
        run_id: run_id,
        event_type: event.event_type,
        seq: event.seq,
        outcome: outcome
      }
    )

    case result do
      {:ok, conn} -> {:ok, conn, emitted_chunks}
      {:closed, conn} -> {:closed, conn, emitted_chunks}
    end
  end

  defp increment_stream_totals(summary, emitted_events, emitted_chunks) do
    summary
    |> Map.update!(:emitted_events, &(&1 + emitted_events))
    |> Map.update!(:emitted_chunks, &(&1 + emitted_chunks))
  end

  defp elapsed_ms(started_at) when is_integer(started_at) do
    (System.monotonic_time() - started_at)
    |> System.convert_time_unit(:native, :millisecond)
    |> max(0)
  end

  defp default_tail_timeout_ms do
    Application.get_env(:openagents_runtime, :stream_tail_timeout_ms, 1_000)
  end

  defp monotonic_ms, do: System.monotonic_time(:millisecond)
end
