defmodule OpenAgentsRuntime.Codex.WorkerSupervisor do
  @moduledoc """
  Dynamic supervisor helpers for Codex worker processes.
  """

  alias OpenAgentsRuntime.Codex.Adapters.InMemory
  alias OpenAgentsRuntime.Codex.WorkerProcess

  @spec ensure_worker(map(), keyword()) :: {:ok, pid()} | {:error, term()}
  def ensure_worker(worker, opts \\ []) when is_map(worker) do
    worker_id = worker.worker_id || worker["worker_id"]

    case Registry.lookup(OpenAgentsRuntime.Codex.WorkerRegistry, worker_id) do
      [{pid, _meta}] when is_pid(pid) ->
        {:ok, pid}

      [] ->
        adapter_module =
          case Keyword.get(opts, :adapter_module) do
            nil -> resolve_adapter(worker.adapter || worker["adapter"])
            module -> module
          end

        child_spec =
          {WorkerProcess,
           [
             worker: worker,
             adapter_module: adapter_module,
             channel_capacity: Keyword.get(opts, :channel_capacity, 128)
           ]}

        DynamicSupervisor.start_child(OpenAgentsRuntime.Codex.WorkerDynamicSupervisor, child_spec)
    end
  end

  defp resolve_adapter("in_memory"), do: InMemory
  defp resolve_adapter(_other), do: InMemory
end
