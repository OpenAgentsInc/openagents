defmodule OpenAgentsRuntime.Khala.Reprojection do
  @moduledoc """
  Drop + replay rebuild operations for runtime-owned Khala projection summaries.

  Rebuild operations clear projector checkpoints and re-run deterministic
  projection from runtime durable history.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Codex.Worker
  alias OpenAgentsRuntime.Khala.ProjectionCheckpoint
  alias OpenAgentsRuntime.Khala.Projector
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Telemetry.Events

  @projection_replay_event [:openagents_runtime, :khala, :projection, :replay]

  @type replay_result :: %{
          scope: String.t(),
          entity_id: String.t(),
          result: String.t(),
          write: String.t() | nil,
          reason: String.t() | nil,
          duration_ms: non_neg_integer()
        }

  @spec rebuild_run(String.t(), keyword()) :: {:ok, replay_result()} | {:error, replay_result()}
  def rebuild_run(run_id, opts \\ []) when is_binary(run_id) do
    rebuild_single("run", "run_summary", run_id, fn -> Projector.project_run(run_id, opts) end)
  end

  @spec rebuild_codex_worker(String.t(), keyword()) ::
          {:ok, replay_result()} | {:error, replay_result()}
  def rebuild_codex_worker(worker_id, opts \\ []) when is_binary(worker_id) do
    rebuild_single("codex_worker", "codex_worker_summary", worker_id, fn ->
      Projector.project_codex_worker(worker_id, opts)
    end)
  end

  @spec rebuild_all(keyword()) :: {:ok, map()}
  def rebuild_all(opts \\ []) do
    run_ids =
      selected_ids(Keyword.get(opts, :run_ids), fn ->
        from(run in Run, select: run.run_id) |> Repo.all()
      end)

    worker_ids =
      selected_ids(Keyword.get(opts, :worker_ids), fn ->
        from(worker in Worker, select: worker.worker_id) |> Repo.all()
      end)

    run_results = Enum.map(run_ids, &normalize_result(rebuild_run(&1, opts)))
    worker_results = Enum.map(worker_ids, &normalize_result(rebuild_codex_worker(&1, opts)))

    results = run_results ++ worker_results
    succeeded = Enum.count(results, &(&1.result == "ok"))
    failed = Enum.count(results, &(&1.result == "error"))

    {:ok,
     %{
       total_entities: length(results),
       succeeded_entities: succeeded,
       failed_entities: failed,
       results: results
     }}
  end

  defp rebuild_single(scope, projection_name, entity_id, projector_fun) do
    started_at = System.monotonic_time(:millisecond)
    clear_checkpoint(projection_name, entity_id)

    result = projector_fun.()
    duration_ms = max(System.monotonic_time(:millisecond) - started_at, 0)

    case result do
      {:ok, projector_result} ->
        emit_projection_replay(scope, "ok", duration_ms, nil)

        {:ok,
         %{
           scope: scope,
           entity_id: entity_id,
           result: "ok",
           write: projector_result.write,
           reason: projector_result.reason,
           duration_ms: duration_ms
         }}

      {:error, reason} ->
        emit_projection_replay(scope, "error", duration_ms, reason)

        {:error,
         %{
           scope: scope,
           entity_id: entity_id,
           result: "error",
           write: nil,
           reason: replay_reason(reason),
           duration_ms: duration_ms
         }}
    end
  end

  defp clear_checkpoint(projection_name, entity_id) do
    from(checkpoint in ProjectionCheckpoint,
      where: checkpoint.projection_name == ^projection_name and checkpoint.entity_id == ^entity_id
    )
    |> Repo.delete_all()
  end

  defp normalize_result({:ok, result}), do: result
  defp normalize_result({:error, result}), do: result

  defp selected_ids(nil, query_fun), do: query_fun.()

  defp selected_ids(ids, _query_fun) when is_list(ids) do
    ids
    |> Enum.filter(&is_binary/1)
    |> Enum.uniq()
  end

  defp selected_ids(id, _query_fun) when is_binary(id), do: [id]
  defp selected_ids(_ids, query_fun), do: query_fun.()

  defp emit_projection_replay(scope, result, duration_ms, reason) do
    Events.emit(
      @projection_replay_event,
      %{count: 1, duration_ms: duration_ms},
      %{scope: scope, result: result, reason_class: replay_reason_class(reason)}
    )
  end

  defp replay_reason(nil), do: nil
  defp replay_reason(reason) when is_atom(reason), do: Atom.to_string(reason)

  defp replay_reason({reason, _detail}) when is_atom(reason),
    do: Atom.to_string(reason)

  defp replay_reason(_reason), do: "error"

  defp replay_reason_class(reason), do: replay_reason(reason) || "none"
end
