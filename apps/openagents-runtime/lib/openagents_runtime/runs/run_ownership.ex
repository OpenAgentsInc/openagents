defmodule OpenAgentsRuntime.Runs.RunOwnership do
  @moduledoc """
  Ownership mapping used to authorize access to run/thread resources.
  """

  use Ecto.Schema

  @primary_key false
  @schema_prefix "runtime"
  schema "run_ownerships" do
    field :run_id, :string, primary_key: true
    field :thread_id, :string, primary_key: true
    field :user_id, :integer
    field :guest_scope, :string

    timestamps(type: :utc_datetime_usec)
  end
end
