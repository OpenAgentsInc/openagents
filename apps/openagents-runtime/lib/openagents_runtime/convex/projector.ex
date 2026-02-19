defmodule OpenAgentsRuntime.Convex.Projector do
  @moduledoc """
  Runtime-owned projector for Convex summary documents.

  This module projects durable runtime state into Convex read-model summaries for:

  - run cards (`runtime/run_summary:<run_id>`)
  - codex worker cards (`runtime/codex_worker_summary:<worker_id>`)
  """

  import Ecto.Query

  require Logger

  alias OpenAgentsRuntime.Codex.Worker
  alias OpenAgentsRuntime.Codex.WorkerEvent
  alias OpenAgentsRuntime.Convex.NoopSink
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent
  alias OpenAgentsRuntime.Telemetry.Events

  @projection_write_event [:openagents_runtime, :convex, :projection, :write]
  @default_projection_version "convex_summary_v1"

  @type projection_name :: :run_summary | :codex_worker_summary
  @type project_result :: {:ok, %{document_id: String.t(), summary: map()}} | {:error, term()}

  @spec project_run(String.t(), keyword()) :: project_result()
  def project_run(run_id, opts \\ []) when is_binary(run_id) do
    with %Run{} = run <- Repo.get(Run, run_id) do
      events = list_run_events(run.run_id)
      summary = run_summary(run, events, opts)
      write_projection(:run_summary, run_summary_document_id(run.run_id), summary, opts)
    else
      nil -> {:error, :run_not_found}
    end
  end

  @spec project_codex_worker(String.t(), keyword()) :: project_result()
  def project_codex_worker(worker_id, opts \\ []) when is_binary(worker_id) do
    with %Worker{} = worker <- Repo.get(Worker, worker_id) do
      events = list_worker_events(worker.worker_id)
      summary = codex_worker_summary(worker, events, opts)

      write_projection(
        :codex_worker_summary,
        codex_worker_summary_document_id(worker.worker_id),
        summary,
        opts
      )
    else
      nil -> {:error, :worker_not_found}
    end
  end

  @spec run_summary_document_id(String.t()) :: String.t()
  def run_summary_document_id(run_id) when is_binary(run_id), do: "runtime/run_summary:#{run_id}"

  @spec codex_worker_summary_document_id(String.t()) :: String.t()
  def codex_worker_summary_document_id(worker_id) when is_binary(worker_id) do
    "runtime/codex_worker_summary:#{worker_id}"
  end

  defp run_summary(%Run{} = run, events, opts) do
    projected_at = projected_at(opts)
    latest_event = List.last(events)
    latest_seq = projection_seq(latest_event, run.latest_seq || 0)
    document_id = run_summary_document_id(run.run_id)

    %{
      "document_id" => document_id,
      "kind" => "run_summary",
      "run_id" => run.run_id,
      "thread_id" => run.thread_id,
      "status" => run.status,
      "owner_user_id" => run.owner_user_id,
      "owner_guest_scope" => run.owner_guest_scope,
      "latest_seq" => latest_seq,
      "event_count" => length(events),
      "latest_event_type" => latest_event_type(latest_event),
      "latest_event_at" => latest_event_at(latest_event),
      "terminal_reason_class" => run.terminal_reason_class,
      "terminal_reason" => run.terminal_reason,
      "terminal_at" => maybe_iso8601(run.terminal_at),
      "updated_at" => maybe_iso8601(run.updated_at),
      "runtime_source" => %{
        "entity" => "run",
        "run_id" => run.run_id,
        "seq" => latest_seq
      },
      "projection_version" => projection_version(opts),
      "projected_at" => maybe_iso8601(projected_at)
    }
  end

  defp codex_worker_summary(%Worker{} = worker, events, opts) do
    projected_at = projected_at(opts)
    latest_event = List.last(events)
    latest_seq = projection_seq(latest_event, worker.latest_seq || 0)
    document_id = codex_worker_summary_document_id(worker.worker_id)

    %{
      "document_id" => document_id,
      "kind" => "codex_worker_summary",
      "worker_id" => worker.worker_id,
      "status" => worker.status,
      "owner_user_id" => worker.owner_user_id,
      "owner_guest_scope" => worker.owner_guest_scope,
      "workspace_ref" => worker.workspace_ref,
      "codex_home_ref" => worker.codex_home_ref,
      "adapter" => worker.adapter,
      "metadata" => worker.metadata || %{},
      "latest_seq" => latest_seq,
      "event_count" => length(events),
      "latest_event_type" => latest_event_type(latest_event),
      "latest_event_at" => latest_event_at(latest_event),
      "started_at" => maybe_iso8601(worker.started_at),
      "stopped_at" => maybe_iso8601(worker.stopped_at),
      "last_heartbeat_at" => maybe_iso8601(worker.last_heartbeat_at),
      "updated_at" => maybe_iso8601(worker.updated_at),
      "runtime_source" => %{
        "entity" => "codex_worker",
        "worker_id" => worker.worker_id,
        "seq" => latest_seq
      },
      "projection_version" => projection_version(opts),
      "projected_at" => maybe_iso8601(projected_at)
    }
  end

  defp list_run_events(run_id) do
    from(event in RunEvent, where: event.run_id == ^run_id, order_by: [asc: event.seq])
    |> Repo.all()
  end

  defp list_worker_events(worker_id) do
    from(event in WorkerEvent, where: event.worker_id == ^worker_id, order_by: [asc: event.seq])
    |> Repo.all()
  end

  defp write_projection(projection_name, document_id, summary, opts) do
    sink = sink_module(opts)
    sink_opts = Keyword.get(opts, :sink_opts, [])

    started_at = System.monotonic_time(:millisecond)
    result = invoke_sink(sink, projection_name, document_id, summary, sink_opts)
    duration_ms = max(System.monotonic_time(:millisecond) - started_at, 0)

    case result do
      :ok ->
        emit_projection_write(projection_name, "ok", duration_ms, document_id, nil)
        {:ok, %{document_id: document_id, summary: summary}}

      {:error, reason} ->
        emit_projection_write(projection_name, "error", duration_ms, document_id, reason)

        Logger.error("convex projection write failed",
          projection: projection_name_to_string(projection_name),
          document_id: document_id,
          reason: inspect(reason)
        )

        {:error, {:sink_write_failed, reason}}
    end
  end

  defp invoke_sink(sink, projection_name, document_id, summary, sink_opts) do
    result =
      case projection_name do
        :run_summary -> sink.upsert_run_summary(document_id, summary, sink_opts)
        :codex_worker_summary -> sink.upsert_codex_worker_summary(document_id, summary, sink_opts)
      end

    case result do
      :ok -> :ok
      {:error, reason} -> {:error, reason}
      other -> {:error, {:invalid_sink_result, other}}
    end
  rescue
    exception ->
      {:error, {:sink_exception, exception}}
  catch
    kind, reason ->
      {:error, {:sink_throw, {kind, reason}}}
  end

  defp sink_module(opts) do
    case Keyword.get(opts, :sink) ||
           Application.get_env(:openagents_runtime, :convex_projection_sink) do
      module when is_atom(module) -> module
      _ -> NoopSink
    end
  end

  defp emit_projection_write(projection_name, result, duration_ms, document_id, reason) do
    Events.emit(
      @projection_write_event,
      %{count: 1, duration_ms: duration_ms},
      %{
        projection: projection_name_to_string(projection_name),
        result: result,
        document_id: document_id,
        reason_class: projection_reason_class(reason)
      }
    )
  end

  defp projection_name_to_string(:run_summary), do: "run_summary"
  defp projection_name_to_string(:codex_worker_summary), do: "codex_worker_summary"

  defp projection_reason_class(nil), do: "none"
  defp projection_reason_class(:ok), do: "none"
  defp projection_reason_class(reason) when is_atom(reason), do: Atom.to_string(reason)

  defp projection_reason_class({reason, _detail}) when is_atom(reason),
    do: Atom.to_string(reason)

  defp projection_reason_class(_reason), do: "error"

  defp projection_seq(%{seq: seq}, _fallback) when is_integer(seq) and seq >= 0, do: seq
  defp projection_seq(_, fallback) when is_integer(fallback) and fallback >= 0, do: fallback
  defp projection_seq(_, _), do: 0

  defp latest_event_type(%{event_type: event_type}) when is_binary(event_type), do: event_type
  defp latest_event_type(_), do: nil

  defp latest_event_at(%{inserted_at: %DateTime{} = inserted_at}), do: maybe_iso8601(inserted_at)
  defp latest_event_at(_), do: nil

  defp projection_version(opts) do
    case Keyword.get(opts, :projection_version) ||
           Application.get_env(:openagents_runtime, :convex_projection_version) do
      value when is_binary(value) and value != "" -> value
      _ -> @default_projection_version
    end
  end

  defp projected_at(opts) do
    case Keyword.get(opts, :now) do
      %DateTime{} = datetime -> datetime
      _ -> DateTime.utc_now()
    end
  end

  defp maybe_iso8601(nil), do: nil
  defp maybe_iso8601(%DateTime{} = datetime), do: DateTime.to_iso8601(datetime)
end
