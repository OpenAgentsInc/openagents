defmodule OpenAgentsRuntime.Spend.SpendReservation do
  @moduledoc """
  Durable budget reservation for settlement-boundary tool execution.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  @states ~w(reserved committed released reconcile_required)
  @required_fields ~w(
    authorization_id
    run_id
    tool_call_id
    amount_sats
    state
    reserved_at
    metadata
  )a

  schema "spend_reservations" do
    field :authorization_id, :string
    field :run_id, :string
    field :tool_call_id, :string
    field :amount_sats, :integer
    field :state, :string, default: "reserved"
    field :provider_correlation_id, :string
    field :provider_idempotency_key, :string
    field :failure_reason, :string
    field :metadata, :map, default: %{}
    field :reserved_at, :utc_datetime_usec
    field :committed_at, :utc_datetime_usec
    field :released_at, :utc_datetime_usec
    field :reconciled_at, :utc_datetime_usec

    timestamps(type: :utc_datetime_usec)
  end

  @type t :: %__MODULE__{
          authorization_id: String.t(),
          run_id: String.t(),
          tool_call_id: String.t(),
          amount_sats: integer(),
          state: String.t(),
          provider_correlation_id: String.t() | nil,
          provider_idempotency_key: String.t() | nil,
          failure_reason: String.t() | nil,
          metadata: map(),
          reserved_at: DateTime.t(),
          committed_at: DateTime.t() | nil,
          released_at: DateTime.t() | nil,
          reconciled_at: DateTime.t() | nil
        }

  @spec states() :: [String.t()]
  def states, do: @states

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(reservation, attrs) do
    reservation
    |> cast(attrs, @required_fields ++ optional_fields())
    |> validate_required(@required_fields)
    |> validate_inclusion(:state, @states)
    |> validate_number(:amount_sats, greater_than: 0)
    |> validate_length(:tool_call_id, min: 1)
    |> foreign_key_constraint(:authorization_id, name: :spend_reservations_authorization_id_fkey)
    |> foreign_key_constraint(:run_id, name: :spend_reservations_run_id_fkey)
    |> unique_constraint(
      [:authorization_id, :run_id, :tool_call_id],
      name: :spend_reservations_authorization_id_run_id_tool_call_id_index
    )
  end

  defp optional_fields do
    ~w(
      provider_correlation_id
      provider_idempotency_key
      failure_reason
      committed_at
      released_at
      reconciled_at
    )a
  end
end
