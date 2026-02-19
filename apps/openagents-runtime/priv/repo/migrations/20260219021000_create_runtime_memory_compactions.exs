defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeMemoryCompactions do
  use Ecto.Migration

  def change do
    create table(:memory_compactions, prefix: "runtime") do
      add :run_id,
          references(:runs,
            column: :run_id,
            type: :string,
            prefix: "runtime",
            on_delete: :delete_all
          ),
          null: false

      add :level, :integer, null: false
      add :trigger_type, :string, null: false
      add :status, :string, null: false, default: "succeeded"
      add :input_event_start_seq, :bigint
      add :input_event_end_seq, :bigint
      add :input_event_count, :integer, null: false, default: 0
      add :output_chunk_id, :string
      add :summary_hash, :string
      add :model_name, :string
      add :model_version, :string
      add :token_count_input, :integer
      add :token_count_output, :integer
      add :artifact_uri, :string
      add :metadata, :map, null: false, default: %{}
      add :error_message, :text
      add :started_at, :utc_datetime_usec, null: false
      add :completed_at, :utc_datetime_usec

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:memory_compactions, :memory_compactions_level_valid,
             check: "level = 1",
             prefix: "runtime"
           )

    create constraint(:memory_compactions, :memory_compactions_trigger_type_valid,
             check: "trigger_type IN ('scheduled', 'pressure')",
             prefix: "runtime"
           )

    create constraint(:memory_compactions, :memory_compactions_status_valid,
             check: "status IN ('succeeded', 'failed', 'noop')",
             prefix: "runtime"
           )

    create index(:memory_compactions, [:run_id, :inserted_at], prefix: "runtime")
    create index(:memory_compactions, [:trigger_type, :status, :inserted_at], prefix: "runtime")
  end
end
