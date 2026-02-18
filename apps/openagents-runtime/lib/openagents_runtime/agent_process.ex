defmodule OpenAgentsRuntime.AgentProcess do
  @moduledoc """
  Minimal per-run process that accepts frames and tracks an in-memory cursor.
  """

  use GenServer

  @type state :: %{
          run_id: String.t(),
          last_frame_id: String.t() | nil,
          frame_count: non_neg_integer()
        }

  @spec start_link(String.t()) :: GenServer.on_start()
  def start_link(run_id) when is_binary(run_id) do
    GenServer.start_link(__MODULE__, run_id, name: OpenAgentsRuntime.AgentRegistry.via(run_id))
  end

  @spec ingest_frame(String.t(), String.t()) :: :ok
  def ingest_frame(run_id, frame_id) when is_binary(run_id) and is_binary(frame_id) do
    GenServer.call(OpenAgentsRuntime.AgentRegistry.via(run_id), {:ingest_frame, frame_id})
  end

  @spec snapshot(String.t()) :: state()
  def snapshot(run_id) when is_binary(run_id) do
    GenServer.call(OpenAgentsRuntime.AgentRegistry.via(run_id), :snapshot)
  end

  @impl true
  def init(run_id) do
    {:ok, %{run_id: run_id, last_frame_id: nil, frame_count: 0}}
  end

  @impl true
  def handle_call({:ingest_frame, frame_id}, _from, state) do
    next_state = %{state | last_frame_id: frame_id, frame_count: state.frame_count + 1}
    {:reply, :ok, next_state}
  end

  def handle_call(:snapshot, _from, state) do
    {:reply, state, state}
  end
end
