defmodule OpenAgentsRuntime.Convex.ProjectionCheckpoint do
  @moduledoc """
  Monotonic checkpoint state for runtime-owned Convex projections.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @primary_key false
  @schema_prefix "runtime"

  schema "convex_projection_checkpoints" do
    field :projection_name, :string, primary_key: true
    field :entity_id, :string, primary_key: true
    field :document_id, :string
    field :last_runtime_seq, :integer
    field :projection_version, :string
    field :summary_hash, :string
    field :last_projected_at, :utc_datetime_usec

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(
    projection_name
    entity_id
    document_id
    last_runtime_seq
    projection_version
    summary_hash
    last_projected_at
  )a

  @type t :: %__MODULE__{
          projection_name: String.t(),
          entity_id: String.t(),
          document_id: String.t(),
          last_runtime_seq: non_neg_integer(),
          projection_version: String.t(),
          summary_hash: String.t(),
          last_projected_at: DateTime.t()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(checkpoint, attrs) do
    checkpoint
    |> cast(attrs, @required_fields)
    |> validate_required(@required_fields)
    |> validate_number(:last_runtime_seq, greater_than_or_equal_to: 0)
    |> validate_length(:summary_hash, is: 64)
    |> unique_constraint([:projection_name, :entity_id],
      name: :convex_projection_checkpoints_projection_name_entity_id_index
    )
  end
end
