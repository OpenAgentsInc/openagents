defmodule OpenAgentsRuntime.Skills.SkillSpec do
  @moduledoc """
  Versioned JSON skill specification stored in the runtime registry.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  @primary_key false

  @states ~w(draft validated published deprecated)

  @required_fields ~w(
    skill_id
    version
    name
    description
    instructions_markdown
    compatibility
    allowed_tools
    scripts
    references
    assets
    commercial
    metadata
    state
    content_hash
  )a

  @optional_fields ~w(license submitted_by)a

  schema "skill_specs" do
    field :skill_id, :string
    field :version, :integer
    field :name, :string
    field :description, :string
    field :license, :string
    field :compatibility, :map, default: %{}
    field :instructions_markdown, :string
    field :allowed_tools, {:array, :map}, default: []
    field :scripts, {:array, :map}, default: []
    field :references, {:array, :map}, default: []
    field :assets, {:array, :map}, default: []
    field :commercial, :map, default: %{}
    field :metadata, :map, default: %{}
    field :submitted_by, :string
    field :state, :string, default: "draft"
    field :content_hash, :string

    timestamps(type: :utc_datetime_usec)
  end

  @type t :: %__MODULE__{
          skill_id: String.t(),
          version: pos_integer(),
          name: String.t(),
          description: String.t(),
          license: String.t() | nil,
          compatibility: map(),
          instructions_markdown: String.t(),
          allowed_tools: [map()],
          scripts: [map()],
          references: [map()],
          assets: [map()],
          commercial: map(),
          metadata: map(),
          submitted_by: String.t() | nil,
          state: String.t(),
          content_hash: String.t()
        }

  @spec states() :: [String.t()]
  def states, do: @states

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(skill_spec, attrs) do
    skill_spec
    |> cast(attrs, @required_fields ++ @optional_fields)
    |> validate_required(@required_fields)
    |> validate_number(:version, greater_than: 0)
    |> validate_length(:skill_id, min: 3, max: 128)
    |> validate_length(:name, min: 1, max: 160)
    |> validate_length(:description, min: 1)
    |> validate_length(:instructions_markdown, min: 1)
    |> validate_format(:skill_id, ~r/^[a-zA-Z0-9._:-]+$/)
    |> validate_inclusion(:state, @states)
    |> validate_change(:allowed_tools, fn :allowed_tools, value ->
      if is_list(value), do: [], else: [allowed_tools: "must be an array"]
    end)
    |> validate_change(:scripts, fn :scripts, value ->
      if is_list(value), do: [], else: [scripts: "must be an array"]
    end)
    |> validate_change(:references, fn :references, value ->
      if is_list(value), do: [], else: [references: "must be an array"]
    end)
    |> validate_change(:assets, fn :assets, value ->
      if is_list(value), do: [], else: [assets: "must be an array"]
    end)
    |> unique_constraint([:skill_id, :version], name: :skill_specs_skill_id_version_index)
  end
end
