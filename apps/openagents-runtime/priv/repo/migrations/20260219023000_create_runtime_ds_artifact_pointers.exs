defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeDsArtifactPointers do
  use Ecto.Migration

  def change do
    create table(:ds_artifact_pointers, primary_key: false, prefix: "runtime") do
      add :signature_id, :string, primary_key: true
      add :primary_artifact, :map, null: false, default: %{}
      add :canary_artifact, :map
      add :canary_percent, :integer, null: false, default: 0
      add :rollout_seed, :string, null: false, default: "default"
      add :metadata, :map, null: false, default: %{}

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:ds_artifact_pointers, :ds_artifact_pointers_canary_percent_valid,
             check: "canary_percent >= 0 AND canary_percent <= 100",
             prefix: "runtime"
           )

    create index(:ds_artifact_pointers, [:updated_at], prefix: "runtime")
  end
end
