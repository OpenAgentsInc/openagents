defmodule OpenAgentsRuntime.Codex.Adapter do
  @moduledoc """
  Behaviour for remote Codex worker adapters.

  Day-1 implementation uses an in-memory adapter. A real adapter can bridge
  to codex app-server over stdio JSON-RPC while preserving the same callbacks.
  """

  @type adapter_state :: term()
  @type request_map :: %{required(String.t()) => term()}
  @type response_map :: %{required(String.t()) => term()}

  @callback init(map()) :: {:ok, adapter_state()} | {:error, term()}
  @callback handle_request(adapter_state(), request_map()) ::
              {:ok, response_map(), adapter_state()} | {:error, response_map(), adapter_state()}
  @callback shutdown(adapter_state()) :: :ok
end
