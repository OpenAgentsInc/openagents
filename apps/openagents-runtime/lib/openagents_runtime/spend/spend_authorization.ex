defmodule OpenAgentsRuntime.Spend.SpendAuthorization do
  @moduledoc """
  Durable spending authorization envelope.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  @primary_key {:authorization_id, :string, autogenerate: false}
  @modes ~w(interactive delegated_budget deny delegated_budget_with_threshold)
  @required_fields ~w(authorization_id mode issued_at spent_sats reserved_sats constraints metadata)a

  @optional_fields ~w(
    owner_user_id
    owner_guest_scope
    autopilot_id
    thread_id
    run_id
    max_total_sats
    max_per_call_sats
    max_per_day_sats
    threshold_sats
    expires_at
    revoked_at
  )a

  @budget_fields ~w(
    max_total_sats
    max_per_call_sats
    max_per_day_sats
    threshold_sats
    spent_sats
    reserved_sats
  )a

  schema "spend_authorizations" do
    field :owner_user_id, :integer
    field :owner_guest_scope, :string
    field :autopilot_id, :string
    field :thread_id, :string
    field :run_id, :string
    field :mode, :string, default: "delegated_budget"
    field :max_total_sats, :integer
    field :max_per_call_sats, :integer
    field :max_per_day_sats, :integer
    field :threshold_sats, :integer
    field :spent_sats, :integer, default: 0
    field :reserved_sats, :integer, default: 0
    field :constraints, :map, default: %{}
    field :metadata, :map, default: %{}
    field :issued_at, :utc_datetime_usec
    field :expires_at, :utc_datetime_usec
    field :revoked_at, :utc_datetime_usec

    timestamps(type: :utc_datetime_usec)
  end

  @type t :: %__MODULE__{
          authorization_id: String.t(),
          owner_user_id: integer() | nil,
          owner_guest_scope: String.t() | nil,
          autopilot_id: String.t() | nil,
          thread_id: String.t() | nil,
          run_id: String.t() | nil,
          mode: String.t(),
          max_total_sats: integer() | nil,
          max_per_call_sats: integer() | nil,
          max_per_day_sats: integer() | nil,
          threshold_sats: integer() | nil,
          spent_sats: integer(),
          reserved_sats: integer(),
          constraints: map(),
          metadata: map(),
          issued_at: DateTime.t(),
          expires_at: DateTime.t() | nil,
          revoked_at: DateTime.t() | nil
        }

  @spec modes() :: [String.t()]
  def modes, do: @modes

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(authorization, attrs) do
    authorization
    |> cast(attrs, @required_fields ++ @optional_fields)
    |> validate_required(@required_fields)
    |> validate_inclusion(:mode, @modes)
    |> validate_owner_binding()
    |> validate_threshold_mode()
    |> validate_budget_fields()
    |> unique_constraint(:authorization_id, name: :spend_authorizations_pkey)
    |> foreign_key_constraint(:run_id, name: :spend_authorizations_run_id_fkey)
  end

  defp validate_owner_binding(changeset) do
    user_id = get_field(changeset, :owner_user_id)
    guest_scope = normalize_string(get_field(changeset, :owner_guest_scope))

    cond do
      is_integer(user_id) and is_binary(guest_scope) ->
        add_error(changeset, :owner_guest_scope, "must be blank when owner_user_id is set")

      is_integer(user_id) ->
        changeset

      is_binary(guest_scope) ->
        changeset

      true ->
        add_error(changeset, :owner_user_id, "must provide owner_user_id or owner_guest_scope")
    end
  end

  defp validate_threshold_mode(changeset) do
    mode = get_field(changeset, :mode)
    threshold = get_field(changeset, :threshold_sats)

    cond do
      mode == "delegated_budget_with_threshold" and is_integer(threshold) and threshold >= 0 ->
        changeset

      mode == "delegated_budget_with_threshold" ->
        add_error(changeset, :threshold_sats, "is required for delegated_budget_with_threshold")

      not is_nil(threshold) ->
        add_error(
          changeset,
          :threshold_sats,
          "must be nil unless mode is delegated_budget_with_threshold"
        )

      true ->
        changeset
    end
  end

  defp validate_budget_fields(changeset) do
    Enum.reduce(@budget_fields, changeset, fn field, acc ->
      validate_change(acc, field, fn _, value ->
        cond do
          is_nil(value) -> []
          is_integer(value) and value >= 0 -> []
          true -> [{field, "must be greater than or equal to 0"}]
        end
      end)
    end)
  end

  defp normalize_string(value) when is_binary(value) do
    value
    |> String.trim()
    |> case do
      "" -> nil
      normalized -> normalized
    end
  end

  defp normalize_string(_), do: nil
end
