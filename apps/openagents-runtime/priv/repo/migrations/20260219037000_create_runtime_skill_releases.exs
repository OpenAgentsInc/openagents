defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeSkillReleases do
  use Ecto.Migration

  def change do
    create table(:skill_releases, primary_key: false, prefix: "runtime") do
      add :release_id, :string, primary_key: true
      add :skill_id, :string, null: false
      add :version, :integer, null: false
      add :bundle, :map, null: false, default: %{}
      add :bundle_hash, :string, null: false
      add :compatibility_report, :map, null: false, default: %{}
      add :published_at, :utc_datetime_usec, null: false
      add :metadata, :map, null: false, default: %{}

      timestamps(type: :utc_datetime_usec)
    end

    create unique_index(:skill_releases, [:skill_id, :version], prefix: "runtime")
    create index(:skill_releases, [:published_at], prefix: "runtime")
  end
end
