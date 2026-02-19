defmodule OpenAgentsRuntime.Runs.Reprojection do
  @moduledoc """
  Full reprojection and drift reconciliation for Laravel-facing read models.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.ProjectionAppliedEvent
  alias OpenAgentsRuntime.Runs.ProjectionWatermark
  alias OpenAgentsRuntime.Runs.Projections
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent

  @default_projection_name "laravel_read_models_v1"

  @type run_result :: %{
          run_id: String.t(),
          action: String.t(),
          drift: boolean(),
          drift_reasons: [String.t()],
          runtime_events: non_neg_integer(),
          projected_events: non_neg_integer(),
          runtime_deltas: non_neg_integer(),
          projected_messages: non_neg_integer(),
          watermark: non_neg_integer(),
          latest_seq: non_neg_integer()
        }

  @type summary :: %{
          total_runs: non_neg_integer(),
          processed_runs: non_neg_integer(),
          repaired_runs: non_neg_integer(),
          dry_run: boolean(),
          results: [run_result()]
        }

  @spec reproject(keyword()) :: {:ok, summary()} | {:error, term()}
  def reproject(opts \\ []) do
    dry_run = Keyword.get(opts, :dry_run, false)
    projection_name = Keyword.get(opts, :projection_name, @default_projection_name)
    runs = list_runs(opts)

    results =
      Enum.map(runs, fn run ->
        reproject_single(run, projection_name, dry_run: dry_run)
      end)

    {:ok, summarize(results, dry_run)}
  end

  @spec reconcile(keyword()) :: {:ok, summary()} | {:error, term()}
  def reconcile(opts \\ []) do
    dry_run = Keyword.get(opts, :dry_run, false)
    repair = Keyword.get(opts, :repair, true)
    projection_name = Keyword.get(opts, :projection_name, @default_projection_name)
    runs = list_runs(opts)

    results =
      Enum.map(runs, fn run ->
        run
        |> drift_snapshot(projection_name)
        |> maybe_repair(run, projection_name, dry_run: dry_run, repair: repair)
      end)

    {:ok, summarize(results, dry_run)}
  end

  @spec reconcile_run(String.t(), keyword()) :: {:ok, run_result()} | {:error, term()}
  def reconcile_run(run_id, opts \\ []) when is_binary(run_id) do
    projection_name = Keyword.get(opts, :projection_name, @default_projection_name)
    dry_run = Keyword.get(opts, :dry_run, false)
    repair = Keyword.get(opts, :repair, true)

    case Repo.get(Run, run_id) do
      %Run{} = run ->
        run
        |> drift_snapshot(projection_name)
        |> maybe_repair(run, projection_name, dry_run: dry_run, repair: repair)
        |> then(&{:ok, &1})

      nil ->
        {:error, :run_not_found}
    end
  end

  defp maybe_repair(snapshot, _run, _projection_name, _opts) when snapshot.drift == false do
    %{snapshot | action: "in_sync"}
  end

  defp maybe_repair(snapshot, run, projection_name, opts) do
    if opts[:dry_run] do
      %{snapshot | action: "dry_run_detected_drift"}
    else
      if opts[:repair] do
        repaired =
          case reproject_single(run, projection_name, dry_run: false) do
            %{action: "reprojected"} = result -> result
            _ -> snapshot
          end

        %{repaired | action: "repaired"}
      else
        %{snapshot | action: "drift_detected_no_repair"}
      end
    end
  end

  defp reproject_single(%Run{} = run, projection_name, opts) do
    dry_run = Keyword.get(opts, :dry_run, false)
    snapshot = drift_snapshot(run, projection_name)

    if dry_run do
      %{snapshot | action: "dry_run_reproject"}
    else
      Repo.transaction(fn ->
        clear_projection_state!(projection_name, run.run_id)
        clear_laravel_rows!(run.run_id)
      end)

      case Projections.project_run(run.run_id, projection_name: projection_name) do
        {:ok, _projection} ->
          drift_snapshot(run, projection_name)
          |> Map.put(:action, "reprojected")

        {:error, reason} ->
          %{snapshot | action: "reproject_failed:#{inspect(reason)}"}
      end
    end
  end

  defp drift_snapshot(%Run{} = run, projection_name) do
    runtime_events = runtime_event_count(run.run_id)
    projected_events = projected_event_count(run.run_id)
    runtime_deltas = runtime_delta_count(run.run_id)
    projected_messages = projected_message_count(run.run_id)
    watermark = Projections.watermark_value(projection_name, run.run_id)
    latest_seq = runtime_latest_seq(run.run_id)

    drift_reasons =
      []
      |> maybe_add(runtime_events == projected_events, "event_count_mismatch")
      |> maybe_add(runtime_deltas == projected_messages, "message_count_mismatch")
      |> maybe_add(watermark >= latest_seq, "watermark_behind")

    %{
      run_id: run.run_id,
      action: "analyzed",
      drift: drift_reasons != [],
      drift_reasons: drift_reasons,
      runtime_events: runtime_events,
      projected_events: projected_events,
      runtime_deltas: runtime_deltas,
      projected_messages: projected_messages,
      watermark: watermark,
      latest_seq: latest_seq
    }
  end

  defp clear_projection_state!(projection_name, run_id) do
    from(watermark in ProjectionWatermark,
      where: watermark.projection_name == ^projection_name and watermark.run_id == ^run_id
    )
    |> Repo.delete_all()

    from(marker in ProjectionAppliedEvent,
      where: marker.projection_name == ^projection_name and marker.run_id == ^run_id
    )
    |> Repo.delete_all()
  end

  defp clear_laravel_rows!(run_id) do
    Repo.query!("DELETE FROM public.messages WHERE run_id = $1", [run_id])
    Repo.query!("DELETE FROM public.run_events WHERE run_id = $1", [run_id])
    Repo.query!("DELETE FROM public.runs WHERE id = $1", [run_id])
    :ok
  end

  defp list_runs(opts) do
    query =
      from(run in Run,
        order_by: [asc: run.inserted_at],
        limit: ^Keyword.get(opts, :limit, 500)
      )
      |> maybe_where_run_id(Keyword.get(opts, :run_id))
      |> maybe_where_thread_id(Keyword.get(opts, :thread_id))
      |> maybe_where_since(Keyword.get(opts, :since))
      |> maybe_where_until(Keyword.get(opts, :until))

    Repo.all(query)
  end

  defp runtime_event_count(run_id) do
    from(event in RunEvent, where: event.run_id == ^run_id, select: count())
    |> Repo.one()
    |> normalize_count()
  end

  defp runtime_delta_count(run_id) do
    from(event in RunEvent,
      where: event.run_id == ^run_id and event.event_type == "run.delta",
      select: count()
    )
    |> Repo.one()
    |> normalize_count()
  end

  defp runtime_latest_seq(run_id) do
    from(event in RunEvent, where: event.run_id == ^run_id, select: max(event.seq))
    |> Repo.one()
    |> case do
      value when is_integer(value) and value > 0 -> value
      _ -> 0
    end
  end

  defp projected_event_count(run_id) do
    Repo.query!("SELECT COUNT(*)::BIGINT FROM public.run_events WHERE run_id = $1", [run_id])
    |> extract_count()
  end

  defp projected_message_count(run_id) do
    Repo.query!("SELECT COUNT(*)::BIGINT FROM public.messages WHERE run_id = $1", [run_id])
    |> extract_count()
  end

  defp summarize(results, dry_run) do
    repaired_runs =
      Enum.count(results, fn result ->
        result.action in ["repaired", "reprojected"]
      end)

    %{
      total_runs: length(results),
      processed_runs: length(results),
      repaired_runs: repaired_runs,
      dry_run: dry_run,
      results: results
    }
  end

  defp maybe_where_run_id(query, run_id) when is_binary(run_id) and run_id != "" do
    where(query, [run], run.run_id == ^run_id)
  end

  defp maybe_where_run_id(query, _), do: query

  defp maybe_where_thread_id(query, thread_id) when is_binary(thread_id) and thread_id != "" do
    where(query, [run], run.thread_id == ^thread_id)
  end

  defp maybe_where_thread_id(query, _), do: query

  defp maybe_where_since(query, %DateTime{} = since),
    do: where(query, [run], run.updated_at >= ^since)

  defp maybe_where_since(query, _), do: query

  defp maybe_where_until(query, %DateTime{} = until),
    do: where(query, [run], run.updated_at <= ^until)

  defp maybe_where_until(query, _), do: query

  defp maybe_add(reasons, true, _reason), do: reasons
  defp maybe_add(reasons, false, reason), do: [reason | reasons]

  defp extract_count(%Postgrex.Result{rows: [[count]]}), do: normalize_count(count)
  defp extract_count(_), do: 0

  defp normalize_count(value) when is_integer(value) and value >= 0, do: value

  defp normalize_count(value) when is_binary(value) do
    case Integer.parse(value) do
      {parsed, _} -> max(parsed, 0)
      :error -> 0
    end
  end

  defp normalize_count(_), do: 0
end
