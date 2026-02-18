defmodule OpenAgentsRuntime.Runs.Projections do
  @moduledoc """
  Projection boundary for Laravel-facing read models.
  """

  @spec projection_key(String.t(), non_neg_integer()) :: String.t()
  def projection_key(run_id, seq) when is_binary(run_id) and is_integer(seq) and seq >= 0 do
    run_id <> ":" <> Integer.to_string(seq)
  end
end
