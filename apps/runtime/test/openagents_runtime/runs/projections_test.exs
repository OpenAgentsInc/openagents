defmodule OpenAgentsRuntime.Runs.ProjectionsTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Projections
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent
  alias OpenAgentsRuntime.Runs.RunEvents

  @projection_name "laravel_read_models_v1"

  setup do
    ensure_projection_target_tables!()
    truncate_projection_target_tables!()
    :ok
  end

  test "project_run/2 advances per-run watermark monotonically and is idempotent" do
    run_id = unique_id("projection_run")
    thread_id = unique_id("projection_thread")
    run = insert_runtime_run(run_id, thread_id)

    {:ok, _} = RunEvents.append_event(run_id, "run.started", %{})
    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "hello"})

    {:ok, _} =
      RunEvents.append_event(run_id, "run.finished", %{
        "status" => "succeeded",
        "reason_class" => "completed",
        "reason" => "done"
      })

    assert {:ok, result} = Projections.project_run(run_id, projection_name: @projection_name)
    assert result.applied_count == 3
    assert result.last_seq == 3
    assert Projections.watermark_value(@projection_name, run_id) == 3

    assert count_rows("public.runs", "id = $1", [run_id]) == 1
    assert count_rows("public.run_events", "run_id = $1", [run_id]) == 3
    assert count_rows("public.messages", "run_id = $1", [run_id]) == 1

    assert {:ok, replay} = Projections.project_run(run_id, projection_name: @projection_name)
    assert replay.applied_count == 0
    assert replay.last_seq == 3
    assert Projections.watermark_value(@projection_name, run_id) == 3

    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "again"})

    assert {:ok, incremental} = Projections.project_run(run_id, projection_name: @projection_name)
    assert incremental.applied_count == 1
    assert incremental.last_seq == 4
    assert Projections.watermark_value(@projection_name, run_id) == 4
    assert count_rows("public.messages", "run_id = $1", [run_id]) == 2

    assert run.thread_id == thread_id
  end

  test "project_events/3 handles out-of-order input and skips duplicates safely" do
    run_id = unique_id("projection_order")
    thread_id = unique_id("projection_order_thread")
    run = insert_runtime_run(run_id, thread_id)

    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "one"})
    {:ok, _} = RunEvents.append_event(run_id, "run.delta", %{"delta" => "two"})

    events =
      from(event in RunEvent, where: event.run_id == ^run_id, order_by: [asc: event.seq])
      |> Repo.all()

    assert {:ok, first_projection} =
             Projections.project_events(run, Enum.reverse(events),
               projection_name: @projection_name
             )

    assert first_projection.applied_count == 2
    assert first_projection.last_seq == 2
    assert Projections.watermark_value(@projection_name, run_id) == 2

    assert {:ok, replay_projection} =
             Projections.project_events(run, events, projection_name: @projection_name)

    assert replay_projection.applied_count == 0
    assert replay_projection.skipped_count == 2
    assert replay_projection.last_seq == 2

    assert count_rows("public.run_events", "run_id = $1", [run_id]) == 2
    assert count_rows("public.messages", "run_id = $1", [run_id]) == 2
  end

  defp insert_runtime_run(run_id, thread_id) do
    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "running",
      owner_user_id: 99,
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
