defmodule OpenAgentsRuntime.DS.Compile.CompileService do
  @moduledoc """
  Deterministic compile runner with durable compile/eval report persistence.
  """

  import Ecto.Query

  alias Ecto.Multi
  alias OpenAgentsRuntime.DS.Compile.CompileReport
  alias OpenAgentsRuntime.DS.Compile.DatasetExporter
  alias OpenAgentsRuntime.DS.Compile.EvalReport
  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.Repo

  @default_compiler_version "ds-elixir/0.1.0"
  @default_search_space_size 2

  @type compile_opt ::
          {:receipts, [map()]}
          | {:traces, [map()]}
          | {:dataset_opts, keyword()}
          | {:search_space, [map()]}
          | {:compiler_version, String.t()}
          | {:started_at, DateTime.t()}
          | {:metadata, map()}

  @spec compile(String.t(), [compile_opt()]) ::
          {:ok, map()} | {:error, :invalid_search_space | :empty_dataset | term()}
  def compile(signature_id, opts \\ []) when is_binary(signature_id) and is_list(opts) do
    receipts = Keyword.get(opts, :receipts, [])
    traces = Keyword.get(opts, :traces, [])
    dataset_opts = Keyword.get(opts, :dataset_opts, [])
    compiler_version = Keyword.get(opts, :compiler_version, @default_compiler_version)
    started_at = Keyword.get(opts, :started_at, DateTime.utc_now())
    metadata = normalize_map(Keyword.get(opts, :metadata, %{}))
    search_space = normalize_search_space(signature_id, Keyword.get(opts, :search_space, []))

    with :ok <- validate_search_space(search_space),
         {:ok, dataset} <- DatasetExporter.export(receipts, traces, dataset_opts),
         :ok <- validate_dataset(dataset) do
      job_spec = %{
        signature_id: signature_id,
        compiler_version: compiler_version,
        split: dataset.split,
        split_seed: dataset.split_seed,
        search_space: search_space
      }

      job_hash = Receipts.stable_hash(job_spec)

      case get_existing_report(signature_id, job_hash, dataset.dataset_hash) do
        %CompileReport{} = report ->
          {:ok,
           map_report_output(report, idempotent_replay: true, dataset_hash: dataset.dataset_hash)}

        nil ->
          persist_compile_report(
            signature_id,
            search_space,
            dataset,
            job_spec,
            job_hash,
            compiler_version,
            started_at,
            metadata
          )
      end
    end
  end

  @spec get_report(String.t()) :: map() | nil
  def get_report(report_id) when is_binary(report_id) do
    query =
      from(report in CompileReport,
        where: report.report_id == ^report_id,
        preload: [:eval_reports],
        limit: 1
      )

    case Repo.one(query) do
      nil ->
        nil

      report ->
        map_report_output(report, idempotent_replay: false, dataset_hash: report.dataset_hash)
    end
  end

  @spec list_reports(String.t(), keyword()) :: [map()]
  def list_reports(signature_id, opts \\ []) when is_binary(signature_id) do
    limit = Keyword.get(opts, :limit, 20)

    query =
      from(report in CompileReport,
        where: report.signature_id == ^signature_id,
        order_by: [desc: report.inserted_at],
        preload: [:eval_reports],
        limit: ^limit
      )

    Repo.all(query)
    |> Enum.map(&map_report_output(&1, idempotent_replay: false, dataset_hash: &1.dataset_hash))
  end

  defp persist_compile_report(
         signature_id,
         search_space,
         dataset,
         job_spec,
         job_hash,
         compiler_version,
         started_at,
         metadata
       ) do
    candidate_results = evaluate_candidates(search_space, dataset)
    selected = pick_best_candidate(candidate_results)
    completed_at = DateTime.utc_now()

    report_id =
      "cmp_" <>
        String.slice(
          Receipts.stable_hash(%{
            signature_id: signature_id,
            job_hash: job_hash,
            dataset_hash: dataset.dataset_hash,
            selected: selected.artifact["compiled_id"]
          }),
          0,
          20
        )

    compile_attrs = %{
      report_id: report_id,
      signature_id: signature_id,
      job_hash: job_hash,
      dataset_hash: dataset.dataset_hash,
      compiler_version: compiler_version,
      status: "succeeded",
      job_spec: job_spec,
      selected_artifact: selected.artifact,
      candidate_artifacts: %{"candidates" => Enum.map(candidate_results, & &1.artifact)},
      metrics: %{
        "best_candidate_score" => selected.score,
        "candidate_count" => length(candidate_results),
        "dataset_counts" => dataset.counts
      },
      metadata: metadata,
      started_at: started_at,
      completed_at: completed_at
    }

    multi =
      Multi.new()
      |> Multi.insert(:compile_report, CompileReport.changeset(%CompileReport{}, compile_attrs))
      |> Multi.run(:eval_reports, fn repo, %{compile_report: report} ->
        inserts =
          candidate_results
          |> Enum.flat_map(fn candidate ->
            Enum.map(candidate.split_scores, fn {split, score} ->
              eval_attrs = %{
                compile_report_id: report.id,
                eval_id: eval_id(report.report_id, candidate.artifact["compiled_id"], split),
                split: split,
                artifact_id: candidate.artifact["compiled_id"],
                score: score,
                metrics: %{
                  "score" => score,
                  "strategy_id" => candidate.artifact["strategy_id"],
                  "job_hash" => job_hash,
                  "dataset_hash" => dataset.dataset_hash
                },
                metadata: %{
                  "signature_id" => signature_id,
                  "compiler_version" => compiler_version
                }
              }

              repo.insert(EvalReport.changeset(%EvalReport{}, eval_attrs))
            end)
          end)

        case Enum.find(inserts, &match?({:error, _}, &1)) do
          nil ->
            query =
              from(eval in EvalReport,
                where: eval.compile_report_id == ^report.id,
                order_by: [asc: eval.split, asc: eval.artifact_id]
              )

            {:ok, repo.all(query)}

          {:error, reason} ->
            {:error, reason}
        end
      end)

    case Repo.transaction(multi) do
      {:ok, %{compile_report: report}} ->
        report = Repo.preload(report, :eval_reports)

        {:ok,
         map_report_output(report, idempotent_replay: false, dataset_hash: dataset.dataset_hash)}

      {:error, _operation, reason, _changes_so_far} ->
        {:error, reason}
    end
  end

  defp evaluate_candidates(search_space, dataset) do
    Enum.map(search_space, fn artifact ->
      split_scores = %{
        "train" => score_for_split(artifact["compiled_id"], dataset.splits.train),
        "holdout" => score_for_split(artifact["compiled_id"], dataset.splits.holdout),
        "test" => score_for_split(artifact["compiled_id"], dataset.splits.test)
      }

      score =
        (split_scores["train"] * 0.2 + split_scores["holdout"] * 0.5 + split_scores["test"] * 0.3)
        |> Float.round(6)

      %{artifact: artifact, split_scores: split_scores, score: score}
    end)
  end

  defp pick_best_candidate(candidate_results) do
    Enum.max_by(candidate_results, & &1.score)
  end

  defp score_for_split(compiled_id, examples) do
    count = length(examples)

    if count == 0 do
      0.0
    else
      examples
      |> Enum.map(fn example ->
        "#{compiled_id}|#{example["example_id"]}"
        |> Receipts.stable_hash()
        |> String.slice(0, 8)
        |> Integer.parse(16)
        |> case do
          {value, _} -> rem(value, 1_000) / 1_000
          :error -> 0.0
        end
      end)
      |> Enum.sum()
      |> Kernel./(count)
      |> Float.round(6)
    end
  end

  defp get_existing_report(signature_id, job_hash, dataset_hash) do
    query =
      from(report in CompileReport,
        where:
          report.signature_id == ^signature_id and report.job_hash == ^job_hash and
            report.dataset_hash == ^dataset_hash,
        preload: [:eval_reports],
        limit: 1
      )

    Repo.one(query)
  end

  defp validate_search_space(search_space) when is_list(search_space) do
    if search_space == [] or
         Enum.any?(search_space, fn artifact ->
           not is_binary(artifact["compiled_id"]) or artifact["compiled_id"] == "" or
             not is_binary(artifact["strategy_id"]) or artifact["strategy_id"] == ""
         end) do
      {:error, :invalid_search_space}
    else
      :ok
    end
  end

  defp validate_search_space(_), do: {:error, :invalid_search_space}

  defp validate_dataset(dataset) do
    if dataset.counts.total <= 0 do
      {:error, :empty_dataset}
    else
      :ok
    end
  end

  defp normalize_search_space(signature_id, search_space) when is_list(search_space) do
    search_space =
      if search_space == [] do
        [
          %{"compiled_id" => "compiled:#{signature_id}:direct.v1", "strategy_id" => "direct.v1"},
          %{
            "compiled_id" => "compiled:#{signature_id}:rlm_lite.v1",
            "strategy_id" => "rlm_lite.v1"
          }
        ]
      else
        search_space
      end
      |> Enum.take(@default_search_space_size)

    Enum.map(search_space, fn artifact ->
      artifact = normalize_map(artifact)

      %{
        "compiled_id" => artifact["compiled_id"],
        "strategy_id" => artifact["strategy_id"] || "direct.v1",
        "metadata" => artifact["metadata"] || %{}
      }
    end)
  end

  defp normalize_search_space(_signature_id, _search_space), do: []

  defp eval_id(report_id, artifact_id, split) do
    "eval_" <>
      String.slice(
        Receipts.stable_hash(%{report_id: report_id, artifact_id: artifact_id, split: split}),
        0,
        20
      )
  end

  defp map_report_output(report, opts) do
    eval_reports =
      report.eval_reports
      |> Enum.sort_by(fn eval -> {eval.split, eval.artifact_id} end)
      |> Enum.map(fn eval ->
        %{
          eval_id: eval.eval_id,
          split: eval.split,
          artifact_id: eval.artifact_id,
          score: eval.score,
          metrics: eval.metrics,
          metadata: eval.metadata
        }
      end)

    %{
      report_id: report.report_id,
      signature_id: report.signature_id,
      status: report.status,
      compiled_id:
        (report.selected_artifact && report.selected_artifact["compiled_id"]) ||
          (report.selected_artifact && report.selected_artifact[:compiled_id]),
      strategy_id:
        (report.selected_artifact && report.selected_artifact["strategy_id"]) ||
          (report.selected_artifact && report.selected_artifact[:strategy_id]),
      selected_artifact: report.selected_artifact,
      candidate_artifacts: report.candidate_artifacts,
      job_hash: report.job_hash,
      dataset_hash: Keyword.fetch!(opts, :dataset_hash),
      metrics: report.metrics,
      metadata: report.metadata,
      started_at: report.started_at,
      completed_at: report.completed_at,
      eval_reports: eval_reports,
      idempotent_replay: Keyword.get(opts, :idempotent_replay, false)
    }
  end

  defp normalize_map(%{} = map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), normalize_value(value)}
      {key, value} -> {to_string(key), normalize_value(value)}
    end)
  end

  defp normalize_map(_), do: %{}

  defp normalize_value(%{} = value), do: normalize_map(value)
  defp normalize_value(list) when is_list(list), do: Enum.map(list, &normalize_value/1)
  defp normalize_value(value), do: value
end
