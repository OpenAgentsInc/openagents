defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeProjectionStateTables do
  use Ecto.Migration

  def change do
    create table(:projection_watermarks, primary_key: false, prefix: "runtime") do
      add :projection_name, :string, null: false
      add :run_id, :string, null: false
      add :last_seq, :bigint, null: false, default: 0

      timestamps(type: :utc_datetime_usec)
    end

    create unique_index(:projection_watermarks, [:projection_name, :run_id], prefix: "runtime")
    create index(:projection_watermarks, [:projection_name, :updated_at], prefix: "runtime")

    create table(:projection_applied_events, primary_key: false, prefix: "runtime") do
      add :projection_name, :string, null: false
      add :run_id, :string, null: false
      add :seq, :bigint, null: false
      add :applied_at, :utc_datetime_usec, null: false
    end

    create unique_index(:projection_applied_events, [:projection_name, :run_id, :seq],
             prefix: "runtime"
           )

    create index(:projection_applied_events, [:projection_name, :run_id, :applied_at],
             prefix: "runtime"
           )
  end
end
