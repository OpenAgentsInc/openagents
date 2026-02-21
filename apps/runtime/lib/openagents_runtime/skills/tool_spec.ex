defmodule OpenAgentsRuntime.Skills.ToolSpec do
  @moduledoc """
  Versioned JSON tool specification stored in the runtime registry.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  @primary_key false

  @states ~w(draft validated published deprecated)

  @required_fields ~w(
    tool_id
    version
    tool_pack
    name
    description
    execution_kind
    integration_manifest
    content_hash
    state
  )a

  @optional_fields ~w(
    input_schema
    output_schema
    auth_requirements
    safety_policy
    commercial
    metadata
    submitted_by
  )a

  schema "tool_specs" do
    field :tool_id, :string
    field :version, :integer
    field :tool_pack, :string
    field :name, :string
    field :description, :string
    field :execution_kind, :string, default: "http"
    field :input_schema, :map, default: %{}
    field :output_schema, :map, default: %{}
    field :integration_manifest, :map, default: %{}
    field :auth_requirements, :map, default: %{}
    field :safety_policy, :map, default: %{}
    field :commercial, :map, default: %{}
    field :metadata, :map, default: %{}
    field :submitted_by, :string
    field :state, :string, default: "draft"
    field :content_hash, :string

    timestamps(type: :utc_datetime_usec)
  end

  @type t :: %__MODULE__{
          tool_id: String.t(),
          version: pos_integer(),
          tool_pack: String.t(),
          name: String.t(),
          description: String.t(),
          execution_kind: String.t(),
          input_schema: map(),
          output_schema: map(),
          integration_manifest: map(),
          auth_requirements: map(),
          safety_policy: map(),
          commercial: map(),
          metadata: map(),
          submitted_by: String.t() | nil,
          state: String.t(),
          content_hash: String.t()
        }

  @spec states() :: [String.t()]
  def states, do: @states

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(tool_spec, attrs) do
    tool_spec
    |> cast(attrs, @required_fields ++ @optional_fields)
    |> validate_required(@required_fields)
    |> validate_number(:version, greater_than: 0)
    |> validate_length(:tool_id, min: 3, max: 128)
    |> validate_length(:tool_pack, min: 1, max: 120)
    |> validate_length(:name, min: 1, max: 160)
    |> validate_length(:description, min: 1)
    |> validate_inclusion(:state, @states)
    |> validate_format(:tool_id, ~r/^[a-zA-Z0-9._:-]+$/)
    |> unique_constraint([:tool_id, :version], name: :tool_specs_tool_id_version_index)
  end
end
