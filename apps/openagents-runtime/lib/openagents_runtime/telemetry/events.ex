defmodule OpenAgentsRuntime.Telemetry.Events do
  @moduledoc """
  Telemetry emission helper that enriches events with trace/request context.
  """

  alias OpenAgentsRuntime.Security.Sanitizer
  alias OpenAgentsRuntime.Telemetry.Tracing

  @trace_metadata_key_map %{
    "traceparent" => :traceparent,
    "tracestate" => :tracestate,
    "x-request-id" => :x_request_id
  }

  @preserve_metadata_keys [
    :run_id,
    :thread_id,
    :frame_id,
    :tool_call_id,
    :status,
    :status_class,
    :reason_class,
    :result,
    :class,
    :reason_code,
    :decision,
    :authorization_mode,
    :settlement_boundary,
    :outcome,
    :action,
    :phase,
    :state,
    :event,
    :provider,
    :fallback_provider,
    :event_type,
    :workflow_id,
    :tool_pack,
    :extension_id,
    :duplicate,
    :cursor,
    :initial_cursor,
    :final_cursor,
    :seq,
    :component,
    :span,
    :traceparent,
    :tracestate,
    :x_request_id
  ]

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

    metadata
    |> Map.merge(trace_metadata)
    |> Sanitizer.sanitize(preserve_keys: @preserve_metadata_keys)
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
