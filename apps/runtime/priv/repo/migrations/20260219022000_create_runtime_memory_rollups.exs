defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeMemoryRollups do
  use Ecto.Migration

  def change do
    create table(:memory_rollups, prefix: "runtime") do
      add :run_id,
          references(:runs,
            column: :run_id,
            type: :string,
            prefix: "runtime",
            on_delete: :delete_all
          ),
          null: false

      add :source_level, :integer, null: false
      add :target_level, :integer, null: false
      add :source_chunk_ids, {:array, :string}, null: false, default: []
      add :output_chunk_id, :string, null: false
      add :summary_hash, :string
      add :status, :string, null: false, default: "succeeded"
      add :metadata, :map, null: false, default: %{}
      add :started_at, :utc_datetime_usec, null: false
      add :completed_at, :utc_datetime_usec
      add :error_message, :text

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:memory_rollups, :memory_rollups_source_level_valid,
             check: "source_level IN (1, 2)",
             prefix: "runtime"
           )

    create constraint(:memory_rollups, :memory_rollups_target_level_valid,
             check: "target_level IN (2, 3)",
             prefix: "runtime"
           )

    create constraint(:memory_rollups, :memory_rollups_status_valid,
             check: "status IN ('succeeded', 'failed', 'noop')",
             prefix: "runtime"
           )

    create unique_index(:memory_rollups, [:run_id, :output_chunk_id], prefix: "runtime")
    create index(:memory_rollups, [:run_id, :target_level, :inserted_at], prefix: "runtime")
  end
end
