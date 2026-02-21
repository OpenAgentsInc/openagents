defmodule OpenAgentsRuntime.Release do
  @moduledoc """
  Release helpers for runtime deploy operations.
  """

  @app :openagents_runtime
  @required_runtime_tables [
    "runtime.sync_stream_events",
    "runtime.sync_topic_sequences",
    "runtime.khala_projection_checkpoints"
  ]

  @spec migrate() :: :ok
  def migrate do
    load_app()

    for repo <- repos() do
      {:ok, _, _} = Ecto.Migrator.with_repo(repo, &Ecto.Migrator.run(&1, :up, all: true))
    end

    :ok
  end

  @spec migrate_and_verify!() :: :ok
  def migrate_and_verify! do
    :ok = migrate()
    :ok = verify_required_tables!()
  end

  @spec verify_required_tables!([String.t()]) :: :ok
  def verify_required_tables!(required_tables \\ @required_runtime_tables)
      when is_list(required_tables) do
    load_app()

    for repo <- repos() do
      {:ok, _, _} =
        Ecto.Migrator.with_repo(repo, fn started_repo ->
          Enum.each(required_tables, fn table ->
            case Ecto.Adapters.SQL.query(started_repo, "SELECT to_regclass($1)::text", [table]) do
              {:ok, %{rows: [[^table]]}} ->
                :ok

              {:ok, %{rows: [[nil]]}} ->
                raise "required runtime table missing after migrations: #{table}"

              {:ok, %{rows: rows}} ->
                raise "unexpected table lookup result for #{table}: #{inspect(rows)}"

              {:error, reason} ->
                raise "failed to verify runtime table #{table}: #{inspect(reason)}"
            end
          end)
        end)
    end

    :ok
  end

  @spec rollback(module(), pos_integer()) :: :ok
  def rollback(repo, version) when is_atom(repo) and is_integer(version) do
    load_app()
    {:ok, _, _} = Ecto.Migrator.with_repo(repo, &Ecto.Migrator.run(&1, :down, to: version))
    :ok
  end

  defp repos do
    Application.fetch_env!(@app, :ecto_repos)
  end

  defp load_app do
    Application.load(@app)
  end
end
