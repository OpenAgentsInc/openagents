defmodule Mix.Tasks.Runtime.Convex.Reproject do
  @shortdoc "Runs Convex projection rebuild jobs from runtime history"
  @moduledoc """
  Runs Convex projection rebuild jobs from runtime durable history.

      mix runtime.convex.reproject --run-id run_123
      mix runtime.convex.reproject --worker-id codexw_123
      mix runtime.convex.reproject --all
  """

  use Mix.Task

  alias OpenAgentsRuntime.Convex.Reprojection

  @switches [run_id: :string, worker_id: :string, all: :boolean]

  @impl Mix.Task
  def run(args) do
    Mix.Task.run("app.start")

    {opts, _argv, _invalid} = OptionParser.parse(args, switches: @switches)

    cond do
      Keyword.get(opts, :all, false) ->
        {:ok, summary} = Reprojection.rebuild_all([])
        print_summary(summary)

      run_id = Keyword.get(opts, :run_id) ->
        print_single_result(Reprojection.rebuild_run(run_id, []))

      worker_id = Keyword.get(opts, :worker_id) ->
        print_single_result(Reprojection.rebuild_codex_worker(worker_id, []))

      true ->
        Mix.raise("provide --run-id <id>, --worker-id <id>, or --all")
    end
  end

  defp print_single_result({:ok, result}) do
    Mix.shell().info(
      "reproject scope=#{result.scope} entity=#{result.entity_id} result=#{result.result} write=#{result.write} duration_ms=#{result.duration_ms}"
    )
  end

  defp print_single_result({:error, result}) do
    Mix.shell().error(
      "reproject scope=#{result.scope} entity=#{result.entity_id} result=#{result.result} reason=#{result.reason} duration_ms=#{result.duration_ms}"
    )
  end

  defp print_summary(summary) do
    Mix.shell().info("Convex reprojection summary:")
    Mix.shell().info("  total_entities: #{summary.total_entities}")
    Mix.shell().info("  succeeded_entities: #{summary.succeeded_entities}")
    Mix.shell().info("  failed_entities: #{summary.failed_entities}")

    Enum.each(summary.results, fn item ->
      Mix.shell().info(
        "  scope=#{item.scope} entity=#{item.entity_id} result=#{item.result} write=#{item.write} reason=#{item.reason} duration_ms=#{item.duration_ms}"
      )
    end)
  end
end
