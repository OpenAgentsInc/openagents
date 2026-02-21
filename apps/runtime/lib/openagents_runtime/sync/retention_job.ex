defmodule OpenAgentsRuntime.Sync.RetentionJob do
  @moduledoc """
  Periodic retention pruning for `runtime.sync_stream_events`.

  Deletes rows older than the configured retention horizon and emits telemetry
  including oldest retained watermark per topic.
  """

  use GenServer

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Sync.StreamEvent
  alias OpenAgentsRuntime.Sync.TopicSequence
  alias OpenAgentsRuntime.Telemetry.Events

  @default_interval_ms 60_000
  @default_horizon_seconds 86_400
  @default_batch_size 5_000

  @cycle_event [:openagents_runtime, :sync, :retention, :cycle]
  @topic_event [:openagents_runtime, :sync, :retention, :topic]

  @type run_once_opt ::
          {:now, DateTime.t()}
          | {:horizon_seconds, non_neg_integer()}
          | {:batch_size, pos_integer()}

  @type run_once_summary :: %{
          deleted: non_neg_integer(),
          cutoff: DateTime.t(),
          oldest_retained: %{String.t() => non_neg_integer() | nil}
        }

  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @spec run_once([run_once_opt()]) :: run_once_summary()
  def run_once(opts \\ []) do
    now = Keyword.get(opts, :now, DateTime.utc_now())
    horizon_seconds = Keyword.get(opts, :horizon_seconds, retention_horizon_seconds())
    batch_size = Keyword.get(opts, :batch_size, retention_batch_size())

    cutoff = DateTime.add(now, -horizon_seconds, :second)
    deleted = delete_expired_rows(cutoff, batch_size)
    oldest_retained = oldest_retained_by_topic()

    emit_cycle_telemetry(deleted)
    emit_topic_telemetry(oldest_retained)

    %{deleted: deleted, cutoff: cutoff, oldest_retained: oldest_retained}
  end

  @impl true
  def init(_opts) do
    state = %{interval_ms: retention_interval_ms()}
    schedule_tick(state.interval_ms)
    {:ok, state}
  end

  @impl true
  def handle_info(:tick, state) do
    _summary = run_once()
    schedule_tick(state.interval_ms)
    {:noreply, state}
  end

  defp delete_expired_rows(cutoff, batch_size) do
    do_delete_expired_rows(cutoff, batch_size, 0)
  end

  defp do_delete_expired_rows(cutoff, batch_size, deleted_so_far) do
    ids =
      from(stream_event in StreamEvent,
        where: stream_event.inserted_at < ^cutoff,
        select: stream_event.id,
        limit: ^batch_size
      )
      |> Repo.all()

    case ids do
      [] ->
        deleted_so_far

      ids ->
        {deleted, _} =
          from(stream_event in StreamEvent, where: stream_event.id in ^ids)
          |> Repo.delete_all()

        do_delete_expired_rows(cutoff, batch_size, deleted_so_far + deleted)
    end
  end

  defp oldest_retained_by_topic do
    topics =
      from(sequence in TopicSequence, select: sequence.topic)
      |> Repo.all()

    oldest_by_topic =
      from(stream_event in StreamEvent,
        group_by: stream_event.topic,
        select: {stream_event.topic, min(stream_event.watermark)}
      )
      |> Repo.all()
      |> Map.new()

    topics
    |> Map.new(fn topic -> {topic, Map.get(oldest_by_topic, topic)} end)
  end

  defp emit_cycle_telemetry(deleted) do
    Events.emit(@cycle_event, %{count: 1, deleted: deleted}, %{component: "sync_retention_job"})
  end

  defp emit_topic_telemetry(oldest_retained) do
    Enum.each(oldest_retained, fn {topic, oldest_watermark} ->
      measurements =
        case oldest_watermark do
          watermark when is_integer(watermark) -> %{count: 1, oldest_watermark: watermark}
          nil -> %{count: 1}
        end

      metadata =
        case oldest_watermark do
          _watermark when is_integer(oldest_watermark) ->
            %{component: "sync_retention_job", event_type: topic, status: "active"}

          nil ->
            %{component: "sync_retention_job", event_type: topic, status: "empty"}
        end

      Events.emit(@topic_event, measurements, metadata)
    end)
  end

  defp schedule_tick(interval_ms) do
    Process.send_after(self(), :tick, interval_ms)
  end

  defp retention_interval_ms do
    Application.get_env(
      :openagents_runtime,
      :khala_sync_retention_interval_ms,
      @default_interval_ms
    )
  end

  defp retention_horizon_seconds do
    Application.get_env(
      :openagents_runtime,
      :khala_sync_retention_horizon_seconds,
      @default_horizon_seconds
    )
  end

  defp retention_batch_size do
    Application.get_env(
      :openagents_runtime,
      :khala_sync_retention_batch_size,
      @default_batch_size
    )
  end
end
