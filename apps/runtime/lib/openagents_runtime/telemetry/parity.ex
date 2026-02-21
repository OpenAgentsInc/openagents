defmodule OpenAgentsRuntime.Telemetry.Parity do
  @moduledoc """
  Parity failure telemetry helpers for OpenClaw-aligned capability classes.
  """

  alias OpenAgentsRuntime.Telemetry.Events

  @type parity_class :: String.t()
  @type reason_class :: String.t()
  @type component :: String.t()
  @type outcome :: String.t()

  @spec emit_failure(parity_class(), reason_class(), component(), outcome(), map()) :: :ok
  def emit_failure(parity_class, reason_class, component, outcome, metadata \\ %{})
      when is_binary(parity_class) and is_binary(reason_class) and is_binary(component) and
             is_binary(outcome) and is_map(metadata) do
    Events.emit(
      [:openagents_runtime, :parity, :failure],
      %{count: 1},
      metadata
      |> Map.put(:class, parity_class)
      |> Map.put(:reason_class, reason_class)
      |> Map.put(:component, component)
      |> Map.put(:outcome, outcome)
    )
  end
end
