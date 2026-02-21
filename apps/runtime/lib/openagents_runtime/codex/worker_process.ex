defmodule OpenAgentsRuntime.Codex.WorkerProcess do
  @moduledoc """
  Stateful process wrapper for a single Codex worker adapter instance.
  """

  use GenServer

  alias OpenAgentsRuntime.Codex.Adapters.InMemory

  @default_timeout 15_000
  @default_channel_capacity 128

  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts) do
    worker = Keyword.fetch!(opts, :worker)
    worker_id = worker.worker_id || worker["worker_id"]

    GenServer.start_link(__MODULE__, opts, name: via(worker_id))
  end

  @spec request(String.t(), map(), timeout()) :: {:ok, map()} | {:error, map()}
  def request(worker_id, request, timeout \\ @default_timeout)
      when is_binary(worker_id) and is_map(request) do
    GenServer.call(via(worker_id), {:request, request}, timeout)
  end

  @spec stop(String.t(), String.t() | nil, timeout()) :: :ok
  def stop(worker_id, reason \\ nil, timeout \\ 5_000) when is_binary(worker_id) do
    GenServer.call(via(worker_id), {:stop, reason}, timeout)
  end

  @spec via(String.t()) :: {:via, Registry, {module(), String.t()}}
  def via(worker_id), do: {:via, Registry, {OpenAgentsRuntime.Codex.WorkerRegistry, worker_id}}

  @impl true
  def init(opts) do
    worker = Keyword.fetch!(opts, :worker)
    adapter_module = Keyword.get(opts, :adapter_module, InMemory)
    channel_capacity = Keyword.get(opts, :channel_capacity, @default_channel_capacity)

    case adapter_module.init(worker) do
      {:ok, adapter_state} ->
        {:ok,
         %{
           worker_id: worker.worker_id,
           adapter_module: adapter_module,
           adapter_state: adapter_state,
           channel_capacity: channel_capacity
         }}

      {:error, reason} ->
        {:stop, reason}
    end
  end

  @impl true
  def handle_call({:request, request}, _from, state) do
    queue_len =
      case Process.info(self(), :message_queue_len) do
        {:message_queue_len, size} when is_integer(size) -> size
        _ -> 0
      end

    if queue_len > state.channel_capacity do
      request_id = request_id(request)

      {:reply,
       {:error,
        %{
          "jsonrpc" => "2.0",
          "id" => request_id,
          "error" => %{"code" => -32001, "message" => "Server overloaded; retry later."}
        }}, state}
    else
      case state.adapter_module.handle_request(state.adapter_state, request) do
        {:ok, response, adapter_state} when is_map(response) ->
          {:reply, {:ok, response}, %{state | adapter_state: adapter_state}}

        {:error, response, adapter_state} when is_map(response) ->
          {:reply, {:error, response}, %{state | adapter_state: adapter_state}}

        {:error, reason, adapter_state} ->
          {:reply, {:error, normalize_adapter_error(request, reason)},
           %{state | adapter_state: adapter_state}}

        other ->
          {:reply, {:error, normalize_adapter_error(request, other)}, state}
      end
    end
  end

  @impl true
  def handle_call({:stop, _reason}, _from, state) do
    _ = state.adapter_module.shutdown(state.adapter_state)
    {:stop, :normal, :ok, state}
  end

  defp normalize_adapter_error(request, reason) do
    %{
      "jsonrpc" => "2.0",
      "id" => request_id(request),
      "error" => %{
        "code" => -32000,
        "message" => "adapter_error",
        "details" => inspect(reason)
      }
    }
  end

  defp request_id(request) when is_map(request) do
    request["request_id"] || request["id"] || "req_unknown"
  end
end
