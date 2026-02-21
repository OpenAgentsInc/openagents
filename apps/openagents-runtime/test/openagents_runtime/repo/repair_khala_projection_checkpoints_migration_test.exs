defmodule OpenAgentsRuntime.Repo.RepairKhalaProjectionCheckpointsMigrationTest do
  use ExUnit.Case, async: true

  @migration_path Path.expand(
                    "../../../priv/repo/migrations/20260220205000_repair_runtime_khala_projection_checkpoints.exs",
                    __DIR__
                  )

  test "repair migration is idempotent and creates runtime checkpoint table" do
    migration = File.read!(@migration_path)

    assert migration =~ "CREATE TABLE IF NOT EXISTS \#{@table}"

    assert migration =~
             "CREATE UNIQUE INDEX IF NOT EXISTS khala_projection_checkpoints_projection_name_entity_id_index"

    assert migration =~
             "CREATE INDEX IF NOT EXISTS khala_projection_checkpoints_projection_name_last_runtime_seq_index"

    assert migration =~
             "CREATE INDEX IF NOT EXISTS khala_projection_checkpoints_document_id_index"
  end
end
