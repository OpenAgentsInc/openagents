defmodule OpenAgentsRuntime.Repo.Migrations.AddHashChainToRunEvents do
  use Ecto.Migration

  def change do
    alter table(:run_events, prefix: "runtime") do
      add :prev_hash, :string
      add :event_hash, :string
    end

    create index(:run_events, [:run_id, :event_hash], prefix: "runtime")
  end
end
