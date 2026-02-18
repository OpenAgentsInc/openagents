defmodule OpenAgentsRuntime.DS.Receipts do
  @moduledoc """
  Receipt shape helper for predict/tool execution.
  """

  @spec new(String.t(), String.t()) :: map()
  def new(run_id, signature_id) when is_binary(run_id) and is_binary(signature_id) do
    %{run_id: run_id, signature_id: signature_id}
  end
end
