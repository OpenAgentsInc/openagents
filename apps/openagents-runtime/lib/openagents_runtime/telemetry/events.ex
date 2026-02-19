defmodule OpenAgentsRuntime.Telemetry.Events do
  @moduledoc """
  Telemetry emission helper that enriches events with trace/request context.
  """

  alias OpenAgentsRuntime.Telemetry.Tracing

  @trace_metadata_key_map %{
    "traceparent" => :traceparent,
    "tracestate" => :tracestate,
    "x-request-id" => :x_request_id
  }

  @type event_name :: [atom()]
  @type measurements :: %{optional(atom()) => number()}
  @type metadata :: %{optional(atom()) => term()}

  @spec emit(event_name(), measurements(), metadata()) :: :ok
  def emit(event_name, measurements \\ %{}, metadata \\ %{})
      when is_list(event_name) and is_map(measurements) and is_map(metadata) do
    :telemetry.execute(
      event_name,
      normalize_measurements(measurements),
      enrich_metadata(metadata)
    )
  end

  @spec enrich_metadata(metadata()) :: metadata()
  def enrich_metadata(metadata) when is_map(metadata) do
    trace_metadata =
      Tracing.current()
      |> Enum.reduce(%{}, fn {key, value}, acc ->
        mapped_key = Map.get(@trace_metadata_key_map, key)

        if mapped_key && is_binary(value) && value != "" do
          Map.put(acc, mapped_key, value)
        else
          acc
        end
      end)

    Map.merge(metadata, trace_metadata)
  end

  defp normalize_measurements(measurements) do
    Enum.reduce(measurements, %{}, fn
      {key, value}, acc when is_number(value) ->
        Map.put(acc, key, value)

      {_key, _value}, acc ->
        acc
    end)
  end
end
