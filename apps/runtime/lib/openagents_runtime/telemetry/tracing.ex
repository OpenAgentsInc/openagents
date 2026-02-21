defmodule OpenAgentsRuntime.Telemetry.Tracing do
  @moduledoc """
  Trace context extraction and propagation for runtime internal requests.

  This module standardizes the trace surface for phases:

  - `ingest`
  - `infer`
  - `tool`
  - `persist`
  - `stream`
  """

  require Logger

  @trace_context_key :openagents_runtime_trace_context
  @trace_headers ~w(traceparent tracestate x-request-id)
  @phase_span_names %{
    ingest: "runtime.ingest",
    infer: "runtime.infer",
    tool: "runtime.tool",
    persist: "runtime.persist",
    stream: "runtime.stream"
  }

  @type phase :: :ingest | :infer | :tool | :persist | :stream
  @type context :: %{optional(String.t()) => String.t()}

  @spec headers() :: [String.t()]
  def headers, do: @trace_headers

  @spec trace_id_header() :: String.t()
  def trace_id_header, do: "traceparent"

  @spec extract_from_conn(Plug.Conn.t()) :: context()
  def extract_from_conn(conn) do
    Enum.reduce(@trace_headers, %{}, fn header, acc ->
      case Plug.Conn.get_req_header(conn, header) do
        [value | _] when is_binary(value) and value != "" -> Map.put(acc, header, value)
        _ -> acc
      end
    end)
  end

  @spec put_current(context()) :: :ok
  def put_current(context) when is_map(context) do
    Process.put(@trace_context_key, context)

    Logger.metadata(
      traceparent: Map.get(context, "traceparent"),
      tracestate: Map.get(context, "tracestate"),
      request_id: Map.get(context, "x-request-id")
    )

    :ok
  end

  @spec current() :: context()
  def current do
    Process.get(@trace_context_key, %{})
  end

  @spec inject_headers(map()) :: map()
  def inject_headers(extra_headers \\ %{}) do
    Map.merge(extra_headers, current())
  end

  @spec span_name(phase()) :: String.t()
  def span_name(phase), do: Map.fetch!(@phase_span_names, phase)

  @spec with_phase_span(phase(), map(), (-> result)) :: result when result: var
  def with_phase_span(phase, metadata, fun) when is_function(fun, 0) and is_map(metadata) do
    with_span(span_name(phase), metadata, fun)
  end

  @spec with_span(String.t(), map(), (-> result)) :: result when result: var
  def with_span(span_name, metadata, fun)
      when is_binary(span_name) and is_map(metadata) and is_function(fun, 0) do
    context = current()

    :telemetry.span(
      [:openagents_runtime, :trace, :span],
      %{system_time: System.system_time()},
      fn ->
        result = fun.()

        {
          result,
          %{span: span_name}
          |> Map.merge(context)
          |> Map.merge(metadata)
        }
      end
    )
  end
end
