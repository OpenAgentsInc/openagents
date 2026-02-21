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
  alias OpenAgentsRuntime.Sync.TopicPolicy
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
          | {:topic_policies, TopicPolicy.topic_policies()}

  @type run_once_summary :: %{
          deleted: non_neg_integer(),
          cutoff: DateTime.t(),
          oldest_retained: %{String.t() => non_neg_integer() | nil},
          topic_stats: %{String.t() => map()}
        }

  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @spec run_once([run_once_opt()]) :: run_once_summary()
  def run_once(opts \\ []) do
    now = Keyword.get(opts, :now, DateTime.utc_now())
    horizon_seconds = Keyword.get(opts, :horizon_seconds, retention_horizon_seconds())

    horizon_override_seconds =
      if Keyword.has_key?(opts, :horizon_seconds), do: horizon_seconds, else: nil

    batch_size = Keyword.get(opts, :batch_size, retention_batch_size())

    topic_policies =
      opts
      |> Keyword.get(:topic_policies, TopicPolicy.topic_policies())
      |> TopicPolicy.topic_policies()

    cutoff = DateTime.add(now, -horizon_seconds, :second)
    topics = sync_topics(topic_policies)

    retention_plan =
      retention_plan(now, topics, topic_policies, horizon_seconds, horizon_override_seconds)

    deleted_by_topic = delete_expired_rows(retention_plan, batch_size)
    oldest_retained = oldest_retained_by_topic(topics)
    head_watermarks = head_watermarks_by_topic(topics)
    topic_stats = topic_stats(retention_plan, deleted_by_topic, oldest_retained, head_watermarks)
    deleted = Enum.reduce(topic_stats, 0, fn {_topic, stats}, acc -> acc + stats.deleted end)

    emit_cycle_telemetry(deleted, map_size(topic_stats))
    emit_topic_telemetry(topic_stats)

    %{
      deleted: deleted,
      cutoff: cutoff,
      oldest_retained: oldest_retained,
      topic_stats: topic_stats
    }
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

  defp sync_topics(topic_policies) do
    sequence_topics = topic_sequence_topics()
    policy_topics = TopicPolicy.known_topics(topic_policies)

    sequence_topics
    |> Kernel.++(policy_topics)
    |> Enum.uniq()
    |> Enum.sort()
  end

  defp topic_sequence_topics do
    from(sequence in TopicSequence, select: sequence.topic)
    |> Repo.all()
  end

  defp retention_plan(
         now,
         topics,
         topic_policies,
         fallback_horizon_seconds,
         horizon_override_seconds
       ) do
    topics
    |> Enum.reduce(%{}, fn topic, acc ->
      policy_retention_seconds =
        TopicPolicy.retention_seconds(topic, topic_policies, fallback_horizon_seconds)

      retention_seconds =
        case horizon_override_seconds do
          value when is_integer(value) and value > 0 -> min(policy_retention_seconds, value)
          _other -> policy_retention_seconds
        end

      cutoff = DateTime.add(now, -retention_seconds, :second)

      Map.put(acc, topic, %{
        cutoff: cutoff,
        retention_seconds: retention_seconds,
        topic_class: TopicPolicy.topic_class(topic, topic_policies),
        compaction_mode: TopicPolicy.compaction_mode(topic, topic_policies),
        snapshot: TopicPolicy.snapshot_metadata(topic, topic_policies)
      })
    end)
  end

  defp delete_expired_rows(retention_plan, batch_size) do
    retention_plan
    |> Enum.map(fn {topic, %{cutoff: cutoff}} ->
      {topic, do_delete_expired_rows(topic, cutoff, batch_size, 0)}
    end)
    |> Map.new()
  end

  defp do_delete_expired_rows(topic, cutoff, batch_size, deleted_so_far) do
    ids =
      from(stream_event in StreamEvent,
        where: stream_event.topic == ^topic and stream_event.inserted_at < ^cutoff,
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

        do_delete_expired_rows(topic, cutoff, batch_size, deleted_so_far + deleted)
    end
  end

  defp oldest_retained_by_topic(topics) do
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

  defp head_watermarks_by_topic(topics) do
    head_by_topic =
      from(stream_event in StreamEvent,
        group_by: stream_event.topic,
        select: {stream_event.topic, max(stream_event.watermark)}
      )
      |> Repo.all()
      |> Map.new()

    topics
    |> Map.new(fn topic -> {topic, Map.get(head_by_topic, topic, 0) || 0} end)
  end

  defp topic_stats(retention_plan, deleted_by_topic, oldest_retained, head_watermarks) do
    retention_plan
    |> Enum.map(fn {topic, plan} ->
      oldest_watermark = Map.get(oldest_retained, topic)
      head_watermark = Map.get(head_watermarks, topic, 0)

      stale_risk =
        case oldest_watermark do
          watermark
          when is_integer(watermark) and watermark > 0 and head_watermark >= watermark ->
            max(head_watermark - watermark, 0)

          _other ->
            0
        end

      {topic,
       %{
         deleted: Map.get(deleted_by_topic, topic, 0),
         cutoff: plan.cutoff,
         retention_seconds: plan.retention_seconds,
         topic_class: plan.topic_class,
         compaction_mode: plan.compaction_mode,
         snapshot: plan.snapshot,
         oldest_watermark: oldest_watermark,
         head_watermark: head_watermark,
         stale_risk: stale_risk
       }}
    end)
    |> Map.new()
  end

  defp emit_cycle_telemetry(deleted, topics) do
    Events.emit(
      @cycle_event,
      %{count: 1, deleted: deleted, topics: topics},
      %{component: "sync_retention_job", status: "ok"}
    )
  end

  defp emit_topic_telemetry(topic_stats) do
    Enum.each(topic_stats, fn {topic, stats} ->
      measurements = %{
        count: 1,
        deleted: stats.deleted,
        retention_seconds: stats.retention_seconds,
        stale_risk: stats.stale_risk
      }

      measurements =
        if is_integer(stats.oldest_watermark) do
          Map.put(measurements, :oldest_watermark, stats.oldest_watermark)
        else
          measurements
        end

      measurements =
        if is_integer(stats.head_watermark) do
          Map.put(measurements, :head_watermark, stats.head_watermark)
        else
          measurements
        end

      metadata = %{
        component: "sync_retention_job",
        event_type: topic,
        status: if(is_integer(stats.oldest_watermark), do: "active", else: "empty"),
        topic_class: stats.topic_class,
        snapshot: if(is_map(stats.snapshot), do: "enabled", else: "disabled")
      }

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
