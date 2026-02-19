defmodule OpenAgentsRuntime.Runs.Run do
  @moduledoc """
  Runtime run record.
  """

  use Ecto.Schema

  @primary_key {:run_id, :string, autogenerate: false}
  @schema_prefix "runtime"
  schema "runs" do
    field :thread_id, :string
    field :status, :string
    field :owner_user_id, :integer
    field :owner_guest_scope, :string
    field :latest_seq, :integer
    field :last_processed_frame_id, :integer
    field :terminal_reason_class, :string
    field :terminal_reason, :string
    field :terminal_at, :utc_datetime_usec

    timestamps(type: :utc_datetime_usec)
  end
end
