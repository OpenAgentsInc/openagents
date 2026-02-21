defmodule OpenAgentsRuntime.Runs.RunLease do
  @moduledoc """
  Lease row for single-executor guarantees per run.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @primary_key {:run_id, :string, autogenerate: false}
  @schema_prefix "runtime"
  schema "run_leases" do
    field :lease_owner, :string
    field :lease_expires_at, :utc_datetime_usec
    field :last_progress_seq, :integer
    field :heartbeat_at, :utc_datetime_usec

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(run_id lease_owner lease_expires_at last_progress_seq heartbeat_at)a

  @type t :: %__MODULE__{
          run_id: String.t(),
          lease_owner: String.t(),
          lease_expires_at: DateTime.t(),
          last_progress_seq: integer(),
          heartbeat_at: DateTime.t()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(lease, attrs) do
    lease
    |> cast(attrs, @required_fields)
    |> validate_required(@required_fields)
    |> validate_number(:last_progress_seq, greater_than_or_equal_to: 0)
  end
end
