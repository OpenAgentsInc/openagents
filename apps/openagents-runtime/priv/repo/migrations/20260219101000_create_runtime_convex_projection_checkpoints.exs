defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeConvexProjectionCheckpoints do
  use Ecto.Migration

  def change do
    create table(:convex_projection_checkpoints, primary_key: false, prefix: "runtime") do
      add :projection_name, :string, null: false
      add :entity_id, :string, null: false
      add :document_id, :string, null: false
      add :last_runtime_seq, :bigint, null: false, default: 0
      add :projection_version, :string, null: false
      add :summary_hash, :string, null: false
      add :last_projected_at, :utc_datetime_usec, null: false

      timestamps(type: :utc_datetime_usec)
    end

    create unique_index(:convex_projection_checkpoints, [:projection_name, :entity_id],
             prefix: "runtime"
           )

    create index(:convex_projection_checkpoints, [:projection_name, :last_runtime_seq],
             prefix: "runtime"
           )

    create index(:convex_projection_checkpoints, [:document_id], prefix: "runtime")
  end
end
