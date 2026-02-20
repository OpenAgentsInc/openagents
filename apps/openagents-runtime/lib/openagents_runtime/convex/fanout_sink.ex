defmodule OpenAgentsRuntime.Convex.FanoutSink do
  @moduledoc """
  Convex sink adapter that fan-outs projector writes to multiple sink modules.

  This enables dual-publish migration lanes (for example Convex + Khala).
  """

  @behaviour OpenAgentsRuntime.Convex.Sink

  alias OpenAgentsRuntime.Convex.NoopSink

  @impl true
  def upsert_run_summary(document_id, summary, opts) do
    fanout(:upsert_run_summary, document_id, summary, opts)
  end

  @impl true
  def upsert_codex_worker_summary(document_id, summary, opts) do
    fanout(:upsert_codex_worker_summary, document_id, summary, opts)
  end

  defp fanout(function_name, document_id, summary, opts) do
    sinks(opts)
    |> Enum.reduce_while(:ok, fn sink, :ok ->
      case invoke(sink, function_name, document_id, summary, opts) do
        :ok -> {:cont, :ok}
        {:error, reason} -> {:halt, {:error, {:fanout_sink_failed, sink, reason}}}
      end
    end)
  end

  defp invoke(sink, function_name, document_id, summary, opts) do
    apply(sink, function_name, [document_id, summary, opts])
  rescue
    exception -> {:error, {:sink_exception, exception}}
  catch
    kind, reason -> {:error, {:sink_throw, {kind, reason}}}
  end

  defp sinks(opts) do
    from_opts = Keyword.get(opts, :sinks)

    configured =
      Application.get_env(:openagents_runtime, :convex_fanout_sink, [])
      |> Keyword.get(:sinks, [NoopSink])

    selected = from_opts || configured

    selected
    |> List.wrap()
    |> Enum.filter(fn sink -> is_atom(sink) and sink != __MODULE__ end)
    |> case do
      [] -> [NoopSink]
      sinks -> sinks
    end
  end
end
