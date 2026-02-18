defmodule OpenAgentsRuntime.Runs.RunStore do
  @moduledoc """
  Minimal run persistence boundary.
  """

  @spec normalize_run_id(String.t()) :: String.t()
  def normalize_run_id(run_id) when is_binary(run_id), do: String.trim(run_id)
end
