defmodule OpenAgentsRuntime.Repo.Migrations.RepairRuntimeKhalaProjectionCheckpoints do
  use Ecto.Migration

  @table "runtime.khala_projection_checkpoints"

  def up do
    execute("CREATE SCHEMA IF NOT EXISTS runtime")

    execute("""
    CREATE TABLE IF NOT EXISTS #{@table} (
      projection_name text NOT NULL,
      entity_id text NOT NULL,
      document_id text NOT NULL,
      last_runtime_seq bigint NOT NULL DEFAULT 0,
      projection_version text NOT NULL,
      summary_hash text NOT NULL,
      last_projected_at timestamp(6) without time zone NOT NULL,
      inserted_at timestamp(6) without time zone NOT NULL DEFAULT timezone('utc', now()),
      updated_at timestamp(6) without time zone NOT NULL DEFAULT timezone('utc', now())
    )
    """)

    execute("""
    CREATE UNIQUE INDEX IF NOT EXISTS khala_projection_checkpoints_projection_name_entity_id_index
    ON #{@table} (projection_name, entity_id)
    """)

    execute("""
    CREATE INDEX IF NOT EXISTS khala_projection_checkpoints_projection_name_last_runtime_seq_index
    ON #{@table} (projection_name, last_runtime_seq)
    """)

    execute("""
    CREATE INDEX IF NOT EXISTS khala_projection_checkpoints_document_id_index
    ON #{@table} (document_id)
    """)
  end

  def down do
    :ok
  end
end
