defmodule Mix.Tasks.Runtime.Reproject do
  @shortdoc "Runs runtime projection reprojection/reconciliation jobs"
  @moduledoc """
  Runs runtime projection reprojection/reconciliation jobs.

      mix runtime.reproject --run-id run_123
      mix runtime.reproject --thread-id thread_abc --dry-run
      mix runtime.reproject --reconcile --no-repair
      mix runtime.reproject --since 2026-02-01T00:00:00Z --until 2026-02-02T00:00:00Z
  """

  use Mix.Task

  alias OpenAgentsRuntime.Runs.Reprojection

  @switches [
    run_id: :string,
    thread_id: :string,
    since: :string,
    until: :string,
    dry_run: :boolean,
    reconcile: :boolean,
    no_repair: :boolean,
    limit: :integer
  ]

  @impl Mix.Task
  def run(args) do
    Mix.Task.run("app.start")

    {opts, _argv, _invalid} = OptionParser.parse(args, switches: @switches)

    opts =
      opts
      |> Keyword.put(:repair, not Keyword.get(opts, :no_repair, false))
      |> Keyword.delete(:no_repair)
      |> put_datetime(:since)
      |> put_datetime(:until)

    result =
      if Keyword.get(opts, :reconcile, false) do
        Reprojection.reconcile(opts)
      else
        Reprojection.reproject(opts)
      end

    {:ok, summary} = result

    Mix.shell().info("Reprojection summary:")
    Mix.shell().info("  total_runs: #{summary.total_runs}")
    Mix.shell().info("  processed_runs: #{summary.processed_runs}")
    Mix.shell().info("  repaired_runs: #{summary.repaired_runs}")
    Mix.shell().info("  dry_run: #{summary.dry_run}")

    Enum.each(summary.results, fn item ->
      Mix.shell().info(
        "  run=#{item.run_id} action=#{item.action} drift=#{item.drift} watermark=#{item.watermark}/#{item.latest_seq}"
      )
    end)
  end

  defp put_datetime(opts, key) do
    case Keyword.get(opts, key) do
      nil ->
        opts

      value when is_binary(value) ->
        case DateTime.from_iso8601(value) do
          {:ok, datetime, _offset} -> Keyword.put(opts, key, datetime)
          {:error, _reason} -> Mix.raise("invalid #{key}: #{value} (expected ISO-8601 UTC)")
        end

      _ ->
        opts
    end
  end
end
