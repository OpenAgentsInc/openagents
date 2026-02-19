defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeDsCompileReports do
  use Ecto.Migration

  def change do
    create table(:ds_compile_reports, prefix: "runtime") do
      add :report_id, :string, null: false
      add :signature_id, :string, null: false
      add :job_hash, :string, null: false
      add :dataset_hash, :string, null: false
      add :compiler_version, :string, null: false, default: "ds-elixir/0.1.0"
      add :status, :string, null: false, default: "succeeded"
      add :job_spec, :map, null: false, default: %{}
      add :selected_artifact, :map
      add :candidate_artifacts, :map, null: false, default: %{}
      add :metrics, :map, null: false, default: %{}
      add :metadata, :map, null: false, default: %{}
      add :started_at, :utc_datetime_usec, null: false
      add :completed_at, :utc_datetime_usec
      add :error_message, :text

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:ds_compile_reports, :ds_compile_reports_status_valid,
             check: "status IN ('succeeded', 'failed')",
             prefix: "runtime"
           )

    create unique_index(:ds_compile_reports, [:report_id], prefix: "runtime")

    create unique_index(
             :ds_compile_reports,
             [:signature_id, :job_hash, :dataset_hash],
             prefix: "runtime",
             name: :ds_compile_reports_signature_job_dataset_index
           )

    create index(:ds_compile_reports, [:signature_id, :inserted_at], prefix: "runtime")

    create table(:ds_eval_reports, prefix: "runtime") do
      add :compile_report_id,
          references(:ds_compile_reports, prefix: "runtime", on_delete: :delete_all),
          null: false

      add :eval_id, :string, null: false
      add :split, :string, null: false
      add :artifact_id, :string, null: false
      add :score, :float, null: false
      add :metrics, :map, null: false, default: %{}
      add :metadata, :map, null: false, default: %{}

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:ds_eval_reports, :ds_eval_reports_split_valid,
             check: "split IN ('train', 'holdout', 'test')",
             prefix: "runtime"
           )

    create unique_index(:ds_eval_reports, [:eval_id], prefix: "runtime")
    create index(:ds_eval_reports, [:compile_report_id, :split], prefix: "runtime")
  end
end
