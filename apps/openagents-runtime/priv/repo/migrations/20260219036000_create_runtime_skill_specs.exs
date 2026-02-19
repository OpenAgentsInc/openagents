defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeSkillSpecs do
  use Ecto.Migration

  def change do
    create table(:skill_specs, primary_key: false, prefix: "runtime") do
      add :skill_id, :string, null: false
      add :version, :integer, null: false
      add :name, :string, null: false
      add :description, :text, null: false
      add :license, :string
      add :compatibility, :map, null: false, default: %{}
      add :instructions_markdown, :text, null: false
      add :allowed_tools, {:array, :map}, null: false, default: []
      add :scripts, {:array, :map}, null: false, default: []
      add :references, {:array, :map}, null: false, default: []
      add :assets, {:array, :map}, null: false, default: []
      add :commercial, :map, null: false, default: %{}
      add :metadata, :map, null: false, default: %{}
      add :submitted_by, :string
      add :state, :string, null: false, default: "draft"
      add :content_hash, :string, null: false

      timestamps(type: :utc_datetime_usec)
    end

    create unique_index(:skill_specs, [:skill_id, :version], prefix: "runtime")
    create index(:skill_specs, [:state], prefix: "runtime")
    create index(:skill_specs, [:inserted_at], prefix: "runtime")
  end
end
