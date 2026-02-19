defmodule OpenAgentsRuntime.Runs.ProjectionAppliedEvent do
  @moduledoc """
  Marker table for idempotent projection application.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @primary_key false
  @schema_prefix "runtime"

  schema "projection_applied_events" do
    field :projection_name, :string, primary_key: true
    field :run_id, :string, primary_key: true
    field :seq, :integer, primary_key: true
    field :applied_at, :utc_datetime_usec
  end

  @required_fields ~w(projection_name run_id seq applied_at)a

  @type t :: %__MODULE__{
          projection_name: String.t(),
          run_id: String.t(),
          seq: pos_integer(),
          applied_at: DateTime.t()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(applied_event, attrs) do
    applied_event
    |> cast(attrs, @required_fields)
    |> validate_required(@required_fields)
    |> validate_number(:seq, greater_than: 0)
    |> unique_constraint([:projection_name, :run_id, :seq],
      name: :projection_applied_events_projection_name_run_id_seq_index
    )
  end
end
