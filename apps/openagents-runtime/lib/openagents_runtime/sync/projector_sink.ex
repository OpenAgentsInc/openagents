defmodule OpenAgentsRuntime.Sync.ProjectorSink do
  @moduledoc """
  Khala projector-compatible sink that writes Khala read models and ordered stream events.

  This sink supports two stream payload modes:

  - `:inline` (default): stream rows include full payload
  - `:pointer`: stream rows omit payload and rely on read-model hydration
  """

  @behaviour OpenAgentsRuntime.Khala.Sink

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Sync.CodexWorkerSummary
  alias OpenAgentsRuntime.Sync.Notifier
  alias OpenAgentsRuntime.Sync.PayloadHash
  alias OpenAgentsRuntime.Sync.RunSummary
  alias OpenAgentsRuntime.Sync.StreamEvent
  alias OpenAgentsRuntime.Sync.WatermarkAllocator
  alias OpenAgentsRuntime.Telemetry.Events

  @projection_write_event [:openagents_runtime, :sync, :projection, :write]
  @stream_append_event [:openagents_runtime, :sync, :stream, :append]

  @run_topic "runtime.run_summaries"
  @worker_topic "runtime.codex_worker_summaries"

  @impl true
  def upsert_run_summary(document_id, summary, opts) do
    persist_projection(
      :run_summary,
      @run_topic,
      RunSummary,
      document_id,
      summary,
      opts
    )
  end

  @impl true
  def upsert_codex_worker_summary(document_id, summary, opts) do
    persist_projection(
      :codex_worker_summary,
      @worker_topic,
      CodexWorkerSummary,
      document_id,
      summary,
      opts
    )
  end

  defp persist_projection(projection, topic, schema_module, document_id, summary, opts) do
    mode = stream_payload_mode(opts)

    with {:ok, doc_version} <- extract_doc_version(summary),
         {:ok, payload_hash} <- payload_hash(summary),
         {:ok, watermark} <- WatermarkAllocator.next(topic),
         :ok <- upsert_read_model(schema_module, document_id, doc_version, summary, payload_hash),
         :ok <-
           append_stream_event(
             topic,
             watermark,
             document_id,
             doc_version,
             summary,
             payload_hash,
             mode
           ) do
      emit_projection_write(projection, mode, "ok")
      :ok
    else
      {:error, reason} ->
        emit_projection_write(projection, mode, "error")
        {:error, reason}
    end
  end

  defp upsert_read_model(schema_module, doc_key, doc_version, payload, payload_hash) do
    now = DateTime.utc_now() |> DateTime.truncate(:microsecond)

    attrs = %{
      doc_key: doc_key,
      doc_version: doc_version,
      payload: payload,
      payload_hash: payload_hash
    }

    changeset =
      schema_module
      |> struct()
      |> schema_module.changeset(attrs)

    case Repo.insert(changeset,
           on_conflict: [
             set: [
               doc_version: doc_version,
               payload: payload,
               payload_hash: payload_hash,
               updated_at: now
             ]
           ],
           conflict_target: :doc_key
         ) do
      {:ok, _row} -> :ok
      {:error, reason} -> {:error, {:read_model_upsert_failed, reason}}
    end
  end

  defp append_stream_event(topic, watermark, doc_key, doc_version, payload, payload_hash, mode) do
    stream_payload = if mode == :pointer, do: nil, else: payload

    attrs = %{
      topic: topic,
      watermark: watermark,
      doc_key: doc_key,
      doc_version: doc_version,
      payload: stream_payload,
      payload_hash: payload_hash
    }

    changeset = StreamEvent.changeset(%StreamEvent{}, attrs)

    case Repo.insert(changeset) do
      {:ok, _stream_event} ->
        _ = Notifier.broadcast_stream_event(topic, watermark)
        emit_stream_append(mode, "ok")
        :ok

      {:error, reason} ->
        emit_stream_append(mode, "error")
        {:error, {:stream_event_insert_failed, reason}}
    end
  end

  defp extract_doc_version(%{"runtime_source" => %{"seq" => seq}})
       when is_integer(seq) and seq >= 0,
       do: {:ok, seq}

  defp extract_doc_version(_summary), do: {:error, :missing_runtime_seq}

  defp payload_hash(summary) when is_map(summary) do
    canonical = PayloadHash.canonical_json(summary)
    {:ok, :crypto.hash(:sha256, canonical)}
  rescue
    exception -> {:error, {:payload_hash_failed, exception}}
  end

  defp payload_hash(_summary), do: {:error, :invalid_summary_payload}

  defp stream_payload_mode(opts) do
    opts_mode = Keyword.get(opts, :stream_payload_mode)

    configured_mode =
      Application.get_env(:openagents_runtime, :khala_sync, [])
      |> Keyword.get(:stream_payload_mode, :inline)

    mode = opts_mode || configured_mode

    if mode in [:inline, :pointer], do: mode, else: :inline
  end

  defp emit_projection_write(projection, mode, result) do
    Events.emit(@projection_write_event, %{count: 1}, %{
      component: "sync_projector_sink",
      projection: projection_name(projection),
      result: result,
      status: mode_name(mode)
    })
  end

  defp emit_stream_append(mode, result) do
    Events.emit(@stream_append_event, %{count: 1}, %{
      component: "sync_projector_sink",
      result: result,
      status: mode_name(mode)
    })
  end

  defp projection_name(:run_summary), do: "run_summary"
  defp projection_name(:codex_worker_summary), do: "codex_worker_summary"

  defp mode_name(:inline), do: "inline"
  defp mode_name(:pointer), do: "pointer"
end
