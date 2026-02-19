defmodule OpenAgentsRuntime.Memory.CompactionJob do
  @moduledoc """
  Scheduled and pressure-triggered compaction job coordinator.
  """

  use GenServer

  import Ecto.Query

  alias OpenAgentsRuntime.Memory.L1Compactor
  alias OpenAgentsRuntime.Memory.TimelineEvent
  alias OpenAgentsRuntime.Repo

  @default_interval_ms 60_000
  @default_batch_size 3
  @default_min_events 30

  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @spec trigger_pressure(String.t(), keyword()) :: {:ok, map()} | {:error, term()}
  def trigger_pressure(run_id, opts \\ []) when is_binary(run_id) do
    GenServer.call(__MODULE__, {:pressure, run_id, opts}, 30_000)
  end

  @impl true
  def init(_opts) do
    state = %{interval_ms: interval_ms()}
    schedule_tick(state.interval_ms)
    {:ok, state}
  end

  @impl true
  def handle_call({:pressure, run_id, opts}, _from, state) do
    reply = L1Compactor.compact_l1(run_id, Keyword.put(opts, :trigger, :pressure))
    {:reply, reply, state}
  end

  @impl true
  def handle_info(:tick, state) do
    run_ids = scheduled_candidates(batch_size(), min_events())

    Enum.each(run_ids, fn run_id ->
      _ = L1Compactor.compact_l1(run_id, trigger: :scheduled, max_events: min_events())
    end)

    schedule_tick(state.interval_ms)
    {:noreply, state}
  end

  defp scheduled_candidates(limit, min_events) do
    query =
      from(event in TimelineEvent,
        group_by: event.run_id,
        having: count(event.id) >= ^min_events,
        order_by: [asc: min(event.inserted_at)],
        select: event.run_id,
        limit: ^limit
      )

    Repo.all(query)
  end

  defp schedule_tick(interval_ms) do
    Process.send_after(self(), :tick, interval_ms)
  end

  defp interval_ms do
    Application.get_env(:openagents_runtime, :l1_compaction_interval_ms, @default_interval_ms)
  end

  defp batch_size do
    Application.get_env(:openagents_runtime, :l1_compaction_batch_size, @default_batch_size)
  end

  defp min_events do
    Application.get_env(:openagents_runtime, :l1_compaction_min_events, @default_min_events)
  end
end
