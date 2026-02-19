defmodule OpenAgentsRuntime.Codex.Worker do
  @moduledoc """
  Durable worker metadata for remote Codex app-server sessions.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  @primary_key {:worker_id, :string, autogenerate: false}

  @required_fields ~w(worker_id adapter status latest_seq metadata started_at)a

  @optional_fields ~w(
    owner_user_id
    owner_guest_scope
    workspace_ref
    codex_home_ref
    stopped_at
    last_heartbeat_at
  )a

  @statuses ~w(starting running stopping stopped failed)

  schema "codex_workers" do
    field :owner_user_id, :integer
    field :owner_guest_scope, :string
    field :workspace_ref, :string
    field :codex_home_ref, :string
    field :adapter, :string, default: "in_memory"
    field :status, :string, default: "running"
    field :latest_seq, :integer, default: 0
    field :metadata, :map, default: %{}
    field :started_at, :utc_datetime_usec
    field :stopped_at, :utc_datetime_usec
    field :last_heartbeat_at, :utc_datetime_usec

    timestamps(type: :utc_datetime_usec)
  end

  @type t :: %__MODULE__{
          worker_id: String.t(),
          owner_user_id: integer() | nil,
          owner_guest_scope: String.t() | nil,
          workspace_ref: String.t() | nil,
          codex_home_ref: String.t() | nil,
          adapter: String.t(),
          status: String.t(),
          latest_seq: non_neg_integer(),
          metadata: map(),
          started_at: DateTime.t(),
          stopped_at: DateTime.t() | nil,
          last_heartbeat_at: DateTime.t() | nil
        }

  @spec statuses() :: [String.t()]
  def statuses, do: @statuses

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(worker, attrs) do
    worker
    |> cast(attrs, @required_fields ++ @optional_fields)
    |> validate_required(@required_fields)
    |> validate_number(:latest_seq, greater_than_or_equal_to: 0)
    |> validate_inclusion(:status, @statuses)
    |> validate_owner_binding()
    |> validate_length(:worker_id, min: 3, max: 160)
    |> validate_format(:worker_id, ~r/^[a-zA-Z0-9._:-]+$/)
  end

  defp validate_owner_binding(changeset) do
    user_id = get_field(changeset, :owner_user_id)
    guest_scope = get_field(changeset, :owner_guest_scope)

    cond do
      is_integer(user_id) and is_binary(guest_scope) and String.trim(guest_scope) != "" ->
        add_error(changeset, :owner_guest_scope, "must be blank when owner_user_id is set")

      is_integer(user_id) ->
        changeset

      is_binary(guest_scope) and String.trim(guest_scope) != "" ->
        changeset

      true ->
        add_error(changeset, :owner_user_id, "must provide owner_user_id or owner_guest_scope")
    end
  end
end
