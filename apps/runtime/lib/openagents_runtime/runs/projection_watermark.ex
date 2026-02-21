defmodule OpenAgentsRuntime.Runs.ProjectionWatermark do
  @moduledoc """
  Per-run projection watermark state.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @primary_key false
  @schema_prefix "runtime"

  schema "projection_watermarks" do
    field :projection_name, :string, primary_key: true
    field :run_id, :string, primary_key: true
    field :last_seq, :integer

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(projection_name run_id last_seq)a

  @type t :: %__MODULE__{
          projection_name: String.t(),
          run_id: String.t(),
          last_seq: non_neg_integer()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(watermark, attrs) do
    watermark
    |> cast(attrs, @required_fields)
    |> validate_required(@required_fields)
    |> validate_number(:last_seq, greater_than_or_equal_to: 0)
    |> unique_constraint([:projection_name, :run_id],
      name: :projection_watermarks_projection_name_run_id_index
    )
  end
end
