defmodule OpenAgentsRuntime.Runs.Projections do
  @moduledoc """
  Runtime-to-Laravel projection writer with idempotent apply semantics and
  monotonic per-run watermarks.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.ProjectionAppliedEvent
  alias OpenAgentsRuntime.Runs.ProjectionWatermark
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent

  @default_projection_name "laravel_read_models_v1"

  @type projection_result :: %{
          applied_count: non_neg_integer(),
          skipped_count: non_neg_integer(),
          last_seq: non_neg_integer()
        }

  @spec projection_key(String.t(), non_neg_integer()) :: String.t()
  def projection_key(run_id, seq) when is_binary(run_id) and is_integer(seq) and seq >= 0 do
    run_id <> ":" <> Integer.to_string(seq)
  end

  @spec project_run(String.t(), keyword()) :: {:ok, projection_result()} | {:error, term()}
  def project_run(run_id, opts \\ []) when is_binary(run_id) do
    projection_name = Keyword.get(opts, :projection_name, @default_projection_name)

    with %Run{} = run <- Repo.get(Run, run_id) do
      watermark = watermark_value(projection_name, run.run_id)

      events =
        from(event in RunEvent,
          where: event.run_id == ^run.run_id and event.seq > ^watermark,
          order_by: [asc: event.seq]
        )
        |> Repo.all()

      project_events(run, events, projection_name: projection_name)
    else
      nil -> {:error, :run_not_found}
    end
  end

  @spec project_events(Run.t(), [RunEvent.t()], keyword()) ::
          {:ok, projection_result()} | {:error, term()}
  def project_events(%Run{} = run, events, opts \\ []) when is_list(events) do
    projection_name = Keyword.get(opts, :projection_name, @default_projection_name)
    last_seq = watermark_value(projection_name, run.run_id)

    events
    |> Enum.sort_by(& &1.seq)
    |> Enum.reduce_while(
      {:ok, %{applied_count: 0, skipped_count: 0, last_seq: last_seq}},
      fn event, {:ok, acc} ->
        case apply_event(projection_name, run, event) do
          {:ok, :applied, seq} ->
            {:cont,
             {:ok,
              %{
                applied_count: acc.applied_count + 1,
                skipped_count: acc.skipped_count,
                last_seq: max(acc.last_seq, seq)
              }}}

          {:ok, :skipped, seq} ->
            {:cont,
             {:ok,
              %{
                applied_count: acc.applied_count,
                skipped_count: acc.skipped_count + 1,
                last_seq: max(acc.last_seq, seq)
              }}}

          {:error, reason} ->
            {:halt, {:error, reason}}
        end
      end
    )
  end

  @spec watermark_value(String.t(), String.t()) :: non_neg_integer()
  def watermark_value(projection_name, run_id)
      when is_binary(projection_name) and is_binary(run_id) do
    query =
      from(watermark in ProjectionWatermark,
        where: watermark.projection_name == ^projection_name and watermark.run_id == ^run_id,
        select: watermark.last_seq,
        limit: 1
      )

    case Repo.one(query) do
      value when is_integer(value) and value >= 0 -> value
      _ -> 0
    end
  end

  defp apply_event(projection_name, %Run{} = run, %RunEvent{} = event) do
    now = DateTime.utc_now()

    Repo.transaction(fn ->
      watermark = lock_watermark!(projection_name, run.run_id, now)

      if event.seq <= watermark.last_seq do
        {:ok, :skipped, watermark.last_seq}
      else
        applied? = mark_applied?(projection_name, run.run_id, event.seq, now)

        if applied? do
          :ok = project_into_laravel_tables(run, event)
          watermark = advance_watermark!(watermark, event.seq, now)
          {:ok, :applied, watermark.last_seq}
        else
          watermark = advance_watermark!(watermark, event.seq, now)
          {:ok, :skipped, watermark.last_seq}
        end
      end
    end)
    |> case do
      {:ok, {:ok, status, seq}} -> {:ok, status, seq}
      {:error, reason} -> {:error, reason}
    end
  end

  defp lock_watermark!(projection_name, run_id, now) do
    query =
      from(watermark in ProjectionWatermark,
        where: watermark.projection_name == ^projection_name and watermark.run_id == ^run_id,
        lock: "FOR UPDATE",
        limit: 1
      )

    case Repo.one(query) do
      %ProjectionWatermark{} = watermark ->
        watermark

      nil ->
        %ProjectionWatermark{}
        |> ProjectionWatermark.changeset(%{
          projection_name: projection_name,
          run_id: run_id,
          last_seq: 0,
          inserted_at: now,
          updated_at: now
        })
        |> Repo.insert!()

        lock_watermark!(projection_name, run_id, now)
    end
  end

  defp mark_applied?(projection_name, run_id, seq, now) do
    {count, _rows} =
      Repo.insert_all(
        ProjectionAppliedEvent,
        [
          %{
            projection_name: projection_name,
            run_id: run_id,
            seq: seq,
            applied_at: now
          }
        ],
        on_conflict: :nothing,
        conflict_target: [:projection_name, :run_id, :seq]
      )

    count == 1
  end

  defp advance_watermark!(watermark, seq, now) do
    next_seq = max(watermark.last_seq || 0, seq)

    watermark
    |> Ecto.Changeset.change(%{last_seq: next_seq, updated_at: now})
    |> Repo.update!()
  end

  defp project_into_laravel_tables(%Run{} = run, %RunEvent{} = event) do
    user_id =
      case run.owner_user_id do
        value when is_integer(value) and value > 0 -> value
        _ -> 0
      end

    status = projected_run_status(run, event)
    started_at = projected_started_at(run, event)
    completed_at = projected_completed_at(run, event)
    run_meta = projected_run_meta(event)
    run_error = projected_run_error(event)

    upsert_run_sql = """
    INSERT INTO public.runs (
      id, thread_id, user_id, status, model_provider, model, usage, meta, error, started_at, completed_at, created_at, updated_at
    ) VALUES ($1, $2, $3, $4, NULL, NULL, NULL, $5, $6, $7, $8, NOW(), NOW())
    ON CONFLICT (id) DO UPDATE SET
      thread_id = EXCLUDED.thread_id,
      user_id = EXCLUDED.user_id,
      status = EXCLUDED.status,
      meta = COALESCE(EXCLUDED.meta, public.runs.meta),
      error = COALESCE(EXCLUDED.error, public.runs.error),
      started_at = COALESCE(public.runs.started_at, EXCLUDED.started_at),
      completed_at = COALESCE(EXCLUDED.completed_at, public.runs.completed_at),
      updated_at = NOW()
    """

    :ok =
      Repo.query!(
        upsert_run_sql,
        [
          run.run_id,
          run.thread_id,
          user_id,
          status,
          run_meta,
          run_error,
          started_at,
          completed_at
        ]
      )
      |> then(fn _ -> :ok end)

    insert_run_event_sql = """
    INSERT INTO public.run_events (
      thread_id, run_id, user_id, type, payload, created_at
    ) VALUES ($1, $2, $3, $4, $5, $6)
    """

    :ok =
      Repo.query!(
        insert_run_event_sql,
        [
          run.thread_id,
          run.run_id,
          user_id,
          event.event_type,
          event.payload || %{},
          event.inserted_at
        ]
      )
      |> then(fn _ -> :ok end)

    maybe_insert_message(run, event, user_id)
  end

  defp maybe_insert_message(%Run{} = run, %RunEvent{event_type: "run.delta"} = event, user_id) do
    delta =
      case event.payload do
        %{"delta" => value} when is_binary(value) -> value
        _ -> nil
      end

    if is_binary(delta) and String.trim(delta) != "" do
      message_id = deterministic_message_id(run.run_id, event.seq)

      sql = """
      INSERT INTO public.messages (
        id, thread_id, run_id, user_id, role, content, meta, created_at, updated_at
      ) VALUES ($1, $2, $3, $4, 'assistant', $5, $6, $7, $7)
      ON CONFLICT (id) DO NOTHING
      """

      Repo.query!(
        sql,
        [
          message_id,
          run.thread_id,
          run.run_id,
          user_id,
          delta,
          %{"runtime_seq" => event.seq, "event_type" => event.event_type},
          event.inserted_at
        ]
      )

      :ok
    else
      :ok
    end
  end

  defp maybe_insert_message(_run, _event, _user_id), do: :ok

  defp projected_run_status(%Run{} = _run, %RunEvent{event_type: "run.started"}), do: "running"

  defp projected_run_status(%Run{} = run, %RunEvent{event_type: "run.finished", payload: payload})
       when is_map(payload) do
    case payload["status"] do
      status when is_binary(status) and status != "" -> status
      _ -> run.status || "running"
    end
  end

  defp projected_run_status(%Run{} = run, _event), do: run.status || "running"

  defp projected_started_at(%Run{} = run, %RunEvent{event_type: "run.started"} = event) do
    run.inserted_at || event.inserted_at
  end

  defp projected_started_at(%Run{} = run, _event), do: run.inserted_at

  defp projected_completed_at(_run, %RunEvent{event_type: "run.finished"} = event),
    do: event.inserted_at

  defp projected_completed_at(_run, _event), do: nil

  defp projected_run_meta(%RunEvent{event_type: "run.finished", payload: payload})
       when is_map(payload) do
    %{"terminal" => Map.take(payload, ["status", "reason_class", "reason"])}
  end

  defp projected_run_meta(%RunEvent{} = event) do
    %{"last_event_type" => event.event_type, "last_seq" => event.seq}
  end

  defp projected_run_error(%RunEvent{event_type: "run.finished", payload: payload})
       when is_map(payload) do
    case payload["status"] do
      "failed" -> payload["reason"] || payload["reason_class"]
      _ -> nil
    end
  end

  defp projected_run_error(_event), do: nil

  defp deterministic_message_id(run_id, seq) do
    digest =
      :crypto.hash(:sha256, "#{run_id}:#{seq}")
      |> Base.encode16(case: :lower)
      |> binary_part(0, 30)

    "rtmsg_" <> digest
  end
end
