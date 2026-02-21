defmodule OpenAgentsRuntime.Codex.Adapters.InMemory do
  @moduledoc """
  Deterministic in-memory Codex adapter used for development and contract tests.
  """

  @behaviour OpenAgentsRuntime.Codex.Adapter

  @impl true
  def init(worker) when is_map(worker) do
    {:ok, %{worker: worker, request_count: 0}}
  end

  @impl true
  def handle_request(state, request) when is_map(state) and is_map(request) do
    request_id = request["request_id"] || request["id"] || "req_unknown"
    method = request["method"] || "unknown.method"
    params = request["params"] || %{}

    response = %{
      "id" => request_id,
      "jsonrpc" => "2.0",
      "result" => %{
        "status" => "accepted",
        "method" => method,
        "echo" => params,
        "request_count" => state.request_count + 1
      }
    }

    {:ok, response, %{state | request_count: state.request_count + 1}}
  end

  @impl true
  def shutdown(_state), do: :ok
end
