defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeRunEvents do
  use Ecto.Migration

  def change do
    create table(:run_events, prefix: "runtime") do
      add :run_id,
          references(:runs,
            column: :run_id,
            type: :string,
            prefix: "runtime",
            on_delete: :delete_all
          ),
          null: false

      add :seq, :bigint, null: false
      add :event_type, :string, null: false
      add :payload, :map, null: false, default: %{}

      timestamps(type: :utc_datetime_usec, updated_at: false)
    end

    create constraint(:run_events, :run_events_seq_positive,
             check: "seq > 0",
             prefix: "runtime"
           )

    create unique_index(:run_events, [:run_id, :seq], prefix: "runtime")
    create index(:run_events, [:run_id, :inserted_at], prefix: "runtime")
  end
end
