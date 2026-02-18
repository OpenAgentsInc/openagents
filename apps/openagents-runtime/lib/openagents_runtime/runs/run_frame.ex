defmodule OpenAgentsRuntime.Runs.RunFrame do
  @moduledoc """
  Ingested frame row with idempotency key per run.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  schema "frames" do
    field :run_id, :string
    field :frame_id, :string
    field :frame_type, :string
    field :payload, :map
    field :payload_hash, :string
    field :occurred_at, :utc_datetime_usec

    timestamps(type: :utc_datetime_usec, updated_at: false)
  end

  @required_fields ~w(run_id frame_id frame_type payload payload_hash)a

  @type t :: %__MODULE__{
          run_id: String.t(),
          frame_id: String.t(),
          frame_type: String.t(),
          payload: map(),
          payload_hash: String.t(),
          occurred_at: DateTime.t() | nil
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(frame, attrs) do
    frame
    |> cast(attrs, @required_fields ++ [:occurred_at])
    |> validate_required(@required_fields)
    |> validate_length(:frame_id, min: 1)
    |> foreign_key_constraint(:run_id, name: :frames_run_id_fkey)
  end
end
