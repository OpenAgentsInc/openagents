defmodule OpenAgentsRuntime.DS.PointerAudit do
  @moduledoc """
  Durable audit record for DS pointer mutations (promote/rollback/canary updates).
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"

  schema "ds_pointer_audits" do
    field :signature_id, :string
    field :action, :string
    field :actor, :string
    field :reason, :string
    field :metadata, :map
    field :before_pointer, :map
    field :after_pointer, :map
    field :target_audit_id, :integer

    timestamps(type: :utc_datetime_usec, updated_at: false)
  end

  @required_fields ~w(signature_id action metadata)a

  @type t :: %__MODULE__{
          signature_id: String.t(),
          action: String.t(),
          actor: String.t() | nil,
          reason: String.t() | nil,
          metadata: map(),
          before_pointer: map() | nil,
          after_pointer: map() | nil,
          target_audit_id: integer() | nil
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(audit, attrs) do
    audit
    |> cast(
      attrs,
      @required_fields ++ [:actor, :reason, :before_pointer, :after_pointer, :target_audit_id]
    )
    |> validate_required(@required_fields)
    |> validate_inclusion(:action, ["promote", "rollback", "set_canary", "clear_canary"])
  end
end
