defmodule OpenAgentsRuntime.FrameCompactor do
  @moduledoc """
  Backward-compatible boundary for compaction orchestration.
  """

  alias OpenAgentsRuntime.Memory.CompactionJob

  @spec compact(String.t(), keyword()) :: {:ok, map()} | {:error, term()}
  def compact(run_id, opts \\ []) when is_binary(run_id) and is_list(opts) do
    trigger = Keyword.get(opts, :trigger, :pressure)

    case trigger do
      :pressure -> CompactionJob.trigger_pressure(run_id, opts)
      "pressure" -> CompactionJob.trigger_pressure(run_id, opts)
      _ -> CompactionJob.trigger_pressure(run_id, Keyword.put(opts, :trigger, :scheduled))
    end
  end
end
