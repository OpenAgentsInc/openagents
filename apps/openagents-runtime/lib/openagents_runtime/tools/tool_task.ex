defmodule OpenAgentsRuntime.Tools.ToolTask do
  @moduledoc """
  Durable tool task lifecycle record.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  schema "tool_tasks" do
    field :run_id, :string
    field :tool_call_id, :string
    field :tool_name, :string
    field :state, :string
    field :input, :map
    field :output, :map
    field :error_class, :string
    field :error_message, :string
    field :metadata, :map
    field :queued_at, :utc_datetime_usec
    field :running_at, :utc_datetime_usec
    field :streaming_at, :utc_datetime_usec
    field :succeeded_at, :utc_datetime_usec
    field :failed_at, :utc_datetime_usec
    field :canceled_at, :utc_datetime_usec
    field :timed_out_at, :utc_datetime_usec

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(run_id tool_call_id tool_name state input metadata queued_at)a
  @states ~w(queued running streaming succeeded failed canceled timed_out)

  @type state :: String.t()

  @type t :: %__MODULE__{
          run_id: String.t(),
          tool_call_id: String.t(),
          tool_name: String.t(),
          state: state(),
          input: map(),
          output: map() | nil,
          error_class: String.t() | nil,
          error_message: String.t() | nil,
          metadata: map(),
          queued_at: DateTime.t(),
          running_at: DateTime.t() | nil,
          streaming_at: DateTime.t() | nil,
          succeeded_at: DateTime.t() | nil,
          failed_at: DateTime.t() | nil,
          canceled_at: DateTime.t() | nil,
          timed_out_at: DateTime.t() | nil
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(task, attrs) do
    task
    |> cast(
      attrs,
      @required_fields ++ timestamp_fields() ++ [:output, :error_class, :error_message]
    )
    |> validate_required(@required_fields)
    |> validate_inclusion(:state, @states)
    |> validate_length(:tool_call_id, min: 1)
    |> validate_length(:tool_name, min: 1)
    |> foreign_key_constraint(:run_id, name: :tool_tasks_run_id_fkey)
    |> unique_constraint([:run_id, :tool_call_id], name: :tool_tasks_run_id_tool_call_id_index)
  end

  @spec states() :: [state()]
  def states, do: @states

  @spec timestamp_fields() :: [atom()]
  def timestamp_fields do
    [
      :queued_at,
      :running_at,
      :streaming_at,
      :succeeded_at,
      :failed_at,
      :canceled_at,
      :timed_out_at
    ]
  end
end
