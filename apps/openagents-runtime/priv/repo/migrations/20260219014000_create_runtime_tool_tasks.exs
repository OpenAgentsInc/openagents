defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeToolTasks do
  use Ecto.Migration

  def change do
    create table(:tool_tasks, prefix: "runtime") do
      add :run_id,
          references(:runs,
            column: :run_id,
            type: :string,
            prefix: "runtime",
            on_delete: :delete_all
          ),
          null: false

      add :tool_call_id, :string, null: false
      add :tool_name, :string, null: false
      add :state, :string, null: false, default: "queued"
      add :input, :map, null: false, default: %{}
      add :output, :map
      add :error_class, :string
      add :error_message, :text
      add :metadata, :map, null: false, default: %{}
      add :queued_at, :utc_datetime_usec, null: false
      add :running_at, :utc_datetime_usec
      add :streaming_at, :utc_datetime_usec
      add :succeeded_at, :utc_datetime_usec
      add :failed_at, :utc_datetime_usec
      add :canceled_at, :utc_datetime_usec
      add :timed_out_at, :utc_datetime_usec

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:tool_tasks, :tool_tasks_state_valid,
             check:
               "state IN ('queued', 'running', 'streaming', 'succeeded', 'failed', 'canceled', 'timed_out')",
             prefix: "runtime"
           )

    create unique_index(:tool_tasks, [:run_id, :tool_call_id], prefix: "runtime")
    create index(:tool_tasks, [:run_id, :state, :updated_at], prefix: "runtime")
  end
end
