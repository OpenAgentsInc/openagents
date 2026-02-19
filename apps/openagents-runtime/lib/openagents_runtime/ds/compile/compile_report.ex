defmodule OpenAgentsRuntime.DS.Compile.CompileReport do
  @moduledoc """
  Durable compile report for DS artifact search/evaluation runs.
  """

  use Ecto.Schema

  import Ecto.Changeset

  alias OpenAgentsRuntime.DS.Compile.EvalReport

  @schema_prefix "runtime"

  schema "ds_compile_reports" do
    field :report_id, :string
    field :signature_id, :string
    field :job_hash, :string
    field :dataset_hash, :string
    field :compiler_version, :string
    field :status, :string
    field :job_spec, :map
    field :selected_artifact, :map
    field :candidate_artifacts, :map
    field :metrics, :map
    field :metadata, :map
    field :started_at, :utc_datetime_usec
    field :completed_at, :utc_datetime_usec
    field :error_message, :string

    has_many :eval_reports, EvalReport, foreign_key: :compile_report_id

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(report_id signature_id job_hash dataset_hash compiler_version status job_spec candidate_artifacts metrics metadata started_at)a

  @type t :: %__MODULE__{
          report_id: String.t(),
          signature_id: String.t(),
          job_hash: String.t(),
          dataset_hash: String.t(),
          compiler_version: String.t(),
          status: String.t(),
          job_spec: map(),
          selected_artifact: map() | nil,
          candidate_artifacts: map(),
          metrics: map(),
          metadata: map(),
          started_at: DateTime.t(),
          completed_at: DateTime.t() | nil,
          error_message: String.t() | nil
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(report, attrs) do
    report
    |> cast(
      attrs,
      @required_fields ++ [:selected_artifact, :completed_at, :error_message]
    )
    |> validate_required(@required_fields)
    |> validate_inclusion(:status, ["succeeded", "failed"])
    |> unique_constraint(:report_id, name: :ds_compile_reports_report_id_index)
    |> unique_constraint(:job_hash, name: :ds_compile_reports_signature_job_dataset_index)
  end
end
