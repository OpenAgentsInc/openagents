defmodule OpenAgentsRuntime.DS.Compile.EvalReport do
  @moduledoc """
  Per-split candidate evaluation report linked to a compile report.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"

  schema "ds_eval_reports" do
    field :eval_id, :string
    field :split, :string
    field :artifact_id, :string
    field :score, :float
    field :metrics, :map
    field :metadata, :map

    belongs_to :compile_report, OpenAgentsRuntime.DS.Compile.CompileReport

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(compile_report_id eval_id split artifact_id score metrics metadata)a

  @type t :: %__MODULE__{
          compile_report_id: pos_integer(),
          eval_id: String.t(),
          split: String.t(),
          artifact_id: String.t(),
          score: float(),
          metrics: map(),
          metadata: map()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(report, attrs) do
    report
    |> cast(attrs, @required_fields)
    |> validate_required(@required_fields)
    |> validate_inclusion(:split, ["train", "holdout", "test"])
    |> unique_constraint(:eval_id, name: :ds_eval_reports_eval_id_index)
    |> foreign_key_constraint(:compile_report_id)
  end
end
