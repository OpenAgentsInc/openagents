defmodule OpenAgentsRuntime.Skills.SkillRelease do
  @moduledoc """
  Immutable published skill release artifact.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  @primary_key {:release_id, :string, autogenerate: false}

  @required_fields ~w(
    release_id
    skill_id
    version
    bundle
    bundle_hash
    compatibility_report
    published_at
    metadata
  )a

  schema "skill_releases" do
    field :skill_id, :string
    field :version, :integer
    field :bundle, :map, default: %{}
    field :bundle_hash, :string
    field :compatibility_report, :map, default: %{}
    field :published_at, :utc_datetime_usec
    field :metadata, :map, default: %{}

    timestamps(type: :utc_datetime_usec)
  end

  @type t :: %__MODULE__{
          release_id: String.t(),
          skill_id: String.t(),
          version: pos_integer(),
          bundle: map(),
          bundle_hash: String.t(),
          compatibility_report: map(),
          published_at: DateTime.t(),
          metadata: map()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(release, attrs) do
    release
    |> cast(attrs, @required_fields)
    |> validate_required(@required_fields)
    |> validate_number(:version, greater_than: 0)
    |> validate_length(:skill_id, min: 3, max: 128)
    |> validate_length(:bundle_hash, min: 64, max: 64)
    |> validate_length(:release_id, min: 8, max: 200)
    |> unique_constraint([:skill_id, :version], name: :skill_releases_skill_id_version_index)
  end
end
