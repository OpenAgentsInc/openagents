defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeToolSpecs do
  use Ecto.Migration

  def change do
    create table(:tool_specs, primary_key: false, prefix: "runtime") do
      add :tool_id, :string, null: false
      add :version, :integer, null: false
      add :tool_pack, :string, null: false
      add :name, :string, null: false
      add :description, :text, null: false
      add :execution_kind, :string, null: false, default: "http"
      add :input_schema, :map, null: false, default: %{}
      add :output_schema, :map, null: false, default: %{}
      add :integration_manifest, :map, null: false, default: %{}
      add :auth_requirements, :map, null: false, default: %{}
      add :safety_policy, :map, null: false, default: %{}
      add :commercial, :map, null: false, default: %{}
      add :metadata, :map, null: false, default: %{}
      add :submitted_by, :string
      add :state, :string, null: false, default: "draft"
      add :content_hash, :string, null: false

      timestamps(type: :utc_datetime_usec)
    end

    create unique_index(:tool_specs, [:tool_id, :version], prefix: "runtime")
    create index(:tool_specs, [:tool_pack, :state], prefix: "runtime")
    create index(:tool_specs, [:inserted_at], prefix: "runtime")
  end
end
