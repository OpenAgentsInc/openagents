defmodule OpenAgentsRuntime.Memory.TimelineStore do
  @moduledoc """
  Minimal timeline persistence boundary used by runtime scaffolding.
  """

  @spec build_pointer(String.t(), non_neg_integer()) :: String.t()
  def build_pointer(run_id, seq) when is_binary(run_id) and is_integer(seq) and seq >= 0 do
    "timeline:" <> run_id <> ":" <> Integer.to_string(seq)
  end
end
