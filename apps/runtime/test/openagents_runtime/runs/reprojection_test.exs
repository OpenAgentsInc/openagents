defmodule OpenAgentsRuntime.Runs.ReprojectionTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Projections
  alias OpenAgentsRuntime.Runs.Reprojection
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents

  @projection_name "laravel_read_models_v1"

  setup do
    ensure_projection_target_tables!()
    truncate_projection_target_tables!()
    :ok
  end

  test "reconcile repairs drift by rebuilding run projections" do
    run_id = unique_id("reconcile_run")
    thread_id = unique_id("reconcile_thread")
    insert_runtime_run(run_id, thread_id)

    {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})
    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "hello"})
    {:ok, _} = RunEvents.append_event(run_id, "run.finished", %{"status" => "succeeded"})

    assert {:ok, _} = Projections.project_run(run_id, projection_name: @projection_name)
    assert count_rows("public.run_events", "run_id = $1", [run_id]) == 3

    Repo.query!(
      "DELETE FROM public.run_events WHERE id IN (SELECT id FROM public.run_events WHERE run_id = $1 ORDER BY id DESC LIMIT 1)",
      [run_id]
    )

    assert count_rows("public.run_events", "run_id = $1", [run_id]) == 2

    assert {:ok, summary} = Reprojection.reconcile(run_id: run_id, repair: true)
    assert summary.repaired_runs == 1
    assert summary.total_runs == 1
    assert hd(summary.results).action == "repaired"
    assert count_rows("public.run_events", "run_id = $1", [run_id]) == 3
    assert Projections.watermark_value(@projection_name, run_id) == 3
  end

  test "dry-run reproject reports actions without mutating read models" do
    run_id = unique_id("dry_run")
    thread_id = unique_id("dry_thread")
    insert_runtime_run(run_id, thread_id)

    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "first"})
    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "second"})

    assert {:ok, summary} = Reprojection.reproject(run_id: run_id, dry_run: true)
    assert summary.total_runs == 1
    assert summary.repaired_runs == 0
    assert hd(summary.results).action == "dry_run_reproject"
    assert count_rows("public.run_events", "run_id = $1", [run_id]) == 0
    assert count_rows("public.messages", "run_id = $1", [run_id]) == 0
    assert Projections.watermark_value(@projection_name, run_id) == 0
  end

  defp insert_runtime_run(run_id, thread_id) do
    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "running",
      owner_user_id: 77,
      latest_seq: 0
    })
  end

  defp ensure_projection_target_tables! do
    Repo.query!("""
    CREATE TABLE IF NOT EXISTS public.runs (
      id VARCHAR(36) PRIMARY KEY,
      thread_id VARCHAR(36),
      user_id BIGINT,
      status VARCHAR(32),
      model_provider VARCHAR(255),
      model VARCHAR(255),
      usage JSONB,
      meta JSONB,
      error TEXT,
      started_at TIMESTAMP,
      completed_at TIMESTAMP,
      created_at TIMESTAMP DEFAULT NOW(),
      updated_at TIMESTAMP DEFAULT NOW()
    )
    """)

    Repo.query!("""
    CREATE TABLE IF NOT EXISTS public.messages (
      id VARCHAR(36) PRIMARY KEY,
      thread_id VARCHAR(36),
      run_id VARCHAR(36),
      user_id BIGINT,
      role VARCHAR(25),
      content TEXT,
      meta JSONB,
      created_at TIMESTAMP DEFAULT NOW(),
      updated_at TIMESTAMP DEFAULT NOW()
    )
    """)

    Repo.query!("""
    CREATE TABLE IF NOT EXISTS public.run_events (
      id BIGSERIAL PRIMARY KEY,
      thread_id VARCHAR(36),
      run_id VARCHAR(36),
      user_id BIGINT,
      type VARCHAR(64),
      payload JSONB,
      created_at TIMESTAMP DEFAULT NOW()
    )
    """)

    Repo.query!(
      "CREATE INDEX IF NOT EXISTS run_events_run_id_id_index ON public.run_events (run_id, id)"
    )
  end

  defp truncate_projection_target_tables! do
    Repo.query!("TRUNCATE TABLE public.messages, public.run_events, public.runs RESTART IDENTITY")
  end

  defp count_rows(table_name, where_clause, params) do
    sql = "SELECT COUNT(*)::BIGINT AS count FROM #{table_name} WHERE #{where_clause}"
    result = Repo.query!(sql, params)
    [count] = hd(result.rows)
    count
  end

  defp unique_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end
end
