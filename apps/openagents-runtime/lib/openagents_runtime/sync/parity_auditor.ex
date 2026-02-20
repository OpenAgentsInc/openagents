defmodule OpenAgentsRuntime.Sync.ParityAuditor do
  @moduledoc """
  Dual-publish parity auditor for Khala checkpoints versus Khala read models.

  This auditor samples projected entities, compares normalized summary hashes and
  runtime sequence drift, then emits telemetry for mismatch rate tracking.
  """

  use GenServer

  import Ecto.Query

  alias OpenAgentsRuntime.Khala.ProjectionCheckpoint
  alias OpenAgentsRuntime.Khala.Projector
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Sync.CodexWorkerSummary
  alias OpenAgentsRuntime.Sync.RunSummary
  alias OpenAgentsRuntime.Telemetry.Events
  alias OpenAgentsRuntime.Telemetry.Parity, as: ParityTelemetry

  @default_interval_ms 30_000
  @default_sample_size 200
  @known_projection_names ["run_summary", "codex_worker_summary"]

  @cycle_event [:openagents_runtime, :sync, :parity, :cycle]
  @entity_event [:openagents_runtime, :sync, :parity, :entity]

  @type run_once_opt ::
          {:sample_size, non_neg_integer()}
          | {:enabled, boolean()}
          | {:projection_names, [String.t()]}
          | {:emit_entity_events, boolean()}
          | {:emit_parity_failures, boolean()}

  @type run_once_summary :: %{
          sampled: non_neg_integer(),
          mismatches: non_neg_integer(),
          missing_documents: non_neg_integer(),
          hash_mismatches: non_neg_integer(),
          lag_drift_nonzero: non_neg_integer(),
          mismatch_rate: float(),
          max_abs_lag_drift: non_neg_integer(),
          avg_abs_lag_drift: float(),
          status: :ok | :mismatch | :empty | :disabled
        }

  @type comparison :: %{
          projection_name: String.t(),
          event_type: String.t(),
          status: String.t(),
          reason_class: String.t(),
          lag_drift: integer(),
          abs_lag_drift: non_neg_integer(),
          entity_id: String.t(),
          document_id: String.t()
        }

  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @spec run_once([run_once_opt()]) :: run_once_summary()
  def run_once(opts \\ []) do
    enabled = Keyword.get(opts, :enabled, parity_enabled?())

    if not enabled do
      %{
        sampled: 0,
        mismatches: 0,
        missing_documents: 0,
        hash_mismatches: 0,
        lag_drift_nonzero: 0,
        mismatch_rate: 0.0,
        max_abs_lag_drift: 0,
        avg_abs_lag_drift: 0.0,
        status: :disabled
      }
    else
      sample_size = normalize_sample_size(Keyword.get(opts, :sample_size, parity_sample_size()))
      projection_names = projection_names(opts)
      emit_entity_events = Keyword.get(opts, :emit_entity_events, true)
      emit_parity_failures = Keyword.get(opts, :emit_parity_failures, true)

      comparisons =
        sample_checkpoints(sample_size, projection_names)
        |> Enum.map(&compare_checkpoint/1)

      summary = summarize(comparisons)
      emit_cycle(summary)

      if emit_entity_events do
        Enum.each(comparisons, &emit_entity/1)
      end

      if emit_parity_failures do
        comparisons
        |> Enum.filter(&(&1.status == "mismatch"))
        |> Enum.each(&emit_parity_failure/1)
      end

      summary
    end
  end

  @impl true
  def init(_opts) do
    state = %{interval_ms: parity_interval_ms()}
    schedule_tick(state.interval_ms)
    {:ok, state}
  end

  @impl true
  def handle_info(:tick, state) do
    _summary = run_once()
    schedule_tick(state.interval_ms)
    {:noreply, state}
  end

  defp sample_checkpoints(sample_size, projection_names) do
    from(checkpoint in ProjectionCheckpoint,
      where: checkpoint.projection_name in ^projection_names,
      order_by: [desc: checkpoint.updated_at],
      limit: ^sample_size
    )
    |> Repo.all()
  end

  defp compare_checkpoint(%ProjectionCheckpoint{} = checkpoint) do
    projection_name = checkpoint.projection_name
    event_type = projection_event_type(projection_name)
    reason_data = %{projection_name: projection_name, event_type: event_type}

    case projection_schema(projection_name) do
      nil ->
        mismatch(reason_data, checkpoint, "unsupported_projection", checkpoint.last_runtime_seq)

      schema ->
        case Repo.get(schema, checkpoint.document_id) do
          nil ->
            mismatch(reason_data, checkpoint, "khala_missing", checkpoint.last_runtime_seq)

          row ->
            khala_hash = Projector.summary_hash_for_parity(row.payload || %{})
            hash_match = khala_hash == checkpoint.summary_hash
            lag_drift = checkpoint.last_runtime_seq - row.doc_version
            hash_mismatch = not hash_match
            lag_mismatch = lag_drift != 0

            reason_class =
              cond do
                hash_mismatch and lag_mismatch -> "hash_and_lag_drift"
                hash_mismatch -> "summary_hash_mismatch"
                lag_mismatch -> "lag_drift"
                true -> "none"
              end

            %{
              projection_name: projection_name,
              event_type: event_type,
              status: if(reason_class == "none", do: "ok", else: "mismatch"),
              reason_class: reason_class,
              lag_drift: lag_drift,
              abs_lag_drift: abs(lag_drift),
              entity_id: checkpoint.entity_id,
              document_id: checkpoint.document_id
            }
        end
    end
  end

  defp mismatch(reason_data, checkpoint, reason_class, lag_drift) do
    %{
      projection_name: reason_data.projection_name,
      event_type: reason_data.event_type,
      status: "mismatch",
      reason_class: reason_class,
      lag_drift: lag_drift,
      abs_lag_drift: abs(lag_drift),
      entity_id: checkpoint.entity_id,
      document_id: checkpoint.document_id
    }
  end

  defp summarize(comparisons) do
    sampled = length(comparisons)
    mismatches = Enum.count(comparisons, &(&1.status == "mismatch"))
    missing_documents = Enum.count(comparisons, &(&1.reason_class == "khala_missing"))

    hash_mismatches =
      Enum.count(comparisons, fn comparison ->
        comparison.reason_class in ["summary_hash_mismatch", "hash_and_lag_drift"]
      end)

    lag_drift_nonzero = Enum.count(comparisons, &(&1.abs_lag_drift > 0))
    max_abs_lag_drift = Enum.max([0 | Enum.map(comparisons, & &1.abs_lag_drift)])
    total_abs_lag_drift = Enum.reduce(comparisons, 0, &(&1.abs_lag_drift + &2))

    mismatch_rate =
      if sampled == 0 do
        0.0
      else
        mismatches / sampled
      end

    avg_abs_lag_drift =
      if sampled == 0 do
        0.0
      else
        total_abs_lag_drift / sampled
      end

    %{
      sampled: sampled,
      mismatches: mismatches,
      missing_documents: missing_documents,
      hash_mismatches: hash_mismatches,
      lag_drift_nonzero: lag_drift_nonzero,
      mismatch_rate: mismatch_rate,
      max_abs_lag_drift: max_abs_lag_drift,
      avg_abs_lag_drift: avg_abs_lag_drift,
      status: status_for(sampled, mismatches)
    }
  end

  defp emit_cycle(summary) do
    Events.emit(
      @cycle_event,
      %{
        count: 1,
        sampled: summary.sampled,
        mismatches: summary.mismatches,
        mismatch_rate: summary.mismatch_rate,
        max_abs_lag_drift: summary.max_abs_lag_drift,
        avg_abs_lag_drift: summary.avg_abs_lag_drift
      },
      %{
        component: "sync_parity_auditor",
        status: Atom.to_string(summary.status)
      }
    )
  end

  defp emit_entity(comparison) do
    Events.emit(
      @entity_event,
      %{
        count: 1,
        lag_drift: comparison.lag_drift,
        abs_lag_drift: comparison.abs_lag_drift
      },
      %{
        component: "sync_parity_auditor",
        status: comparison.status,
        reason_class: comparison.reason_class,
        event_type: comparison.event_type
      }
    )
  end

  defp emit_parity_failure(comparison) do
    ParityTelemetry.emit_failure(
      "sync_dual_publish",
      comparison.reason_class,
      "sync_parity_auditor",
      "mismatch",
      %{
        projection: comparison.projection_name,
        event_type: comparison.event_type,
        entity_id: comparison.entity_id,
        document_id: comparison.document_id
      }
    )
  end

  defp status_for(0, _mismatches), do: :empty
  defp status_for(_sampled, 0), do: :ok
  defp status_for(_sampled, _mismatches), do: :mismatch

  defp projection_schema("run_summary"), do: RunSummary
  defp projection_schema("codex_worker_summary"), do: CodexWorkerSummary
  defp projection_schema(_), do: nil

  defp projection_event_type("run_summary"), do: "runtime.run_summaries"
  defp projection_event_type("codex_worker_summary"), do: "runtime.codex_worker_summaries"
  defp projection_event_type(_), do: "runtime.unknown"

  defp projection_names(opts) do
    opts
    |> Keyword.get(:projection_names, parity_projection_names())
    |> List.wrap()
    |> Enum.filter(&(&1 in @known_projection_names))
    |> Enum.uniq()
    |> case do
      [] -> @known_projection_names
      names -> names
    end
  end

  defp schedule_tick(interval_ms) when is_integer(interval_ms) and interval_ms > 0 do
    Process.send_after(self(), :tick, interval_ms)
  end

  defp schedule_tick(_interval_ms), do: :ok

  defp parity_enabled? do
    case Application.get_env(:openagents_runtime, :khala_sync_parity_enabled, false) do
      true -> true
      false -> false
      value when is_binary(value) -> String.downcase(value) in ["1", "true", "yes"]
      _ -> false
    end
  end

  defp parity_interval_ms do
    Application.get_env(
      :openagents_runtime,
      :khala_sync_parity_interval_ms,
      @default_interval_ms
    )
    |> normalize_interval()
  end

  defp parity_sample_size do
    Application.get_env(
      :openagents_runtime,
      :khala_sync_parity_sample_size,
      @default_sample_size
    )
    |> normalize_sample_size()
  end

  defp parity_projection_names do
    Application.get_env(
      :openagents_runtime,
      :khala_sync_parity_projection_names,
      @known_projection_names
    )
    |> List.wrap()
    |> Enum.map(&to_string/1)
    |> Enum.filter(&(&1 in @known_projection_names))
  end

  defp normalize_interval(value) when is_integer(value) and value >= 1_000, do: value
  defp normalize_interval(_), do: @default_interval_ms

  defp normalize_sample_size(value) when is_integer(value) and value >= 0 and value <= 10_000,
    do: value

  defp normalize_sample_size(_), do: @default_sample_size
end
