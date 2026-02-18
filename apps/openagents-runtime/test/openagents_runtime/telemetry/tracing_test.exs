defmodule OpenAgentsRuntime.Telemetry.TracingTest do
  use ExUnit.Case, async: false

  alias OpenAgentsRuntime.Telemetry.Tracing

  setup do
    Process.delete(:openagents_runtime_trace_context)
    :ok
  end

  test "extracts trace headers from conn" do
    conn =
      Plug.Test.conn(:get, "/internal/v1/health")
      |> Plug.Conn.put_req_header("traceparent", "00-abc-def-01")
      |> Plug.Conn.put_req_header("tracestate", "vendor=state")
      |> Plug.Conn.put_req_header("x-request-id", "req_123")

    assert %{
             "traceparent" => "00-abc-def-01",
             "tracestate" => "vendor=state",
             "x-request-id" => "req_123"
           } = Tracing.extract_from_conn(conn)
  end

  test "with_phase_span emits stop metadata with trace context" do
    parent = self()
    handler_id = "trace-test-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        handler_id,
        [:openagents_runtime, :trace, :span, :stop],
        fn _event_name, _measurements, metadata, _config ->
          send(parent, {:trace_span_stop, metadata})
        end,
        nil
      )

    on_exit(fn -> :telemetry.detach(handler_id) end)

    :ok = Tracing.put_current(%{"traceparent" => "00-parent", "x-request-id" => "req_456"})

    assert :done =
             Tracing.with_phase_span(:stream, %{run_id: "run_1"}, fn ->
               :done
             end)

    assert_receive {:trace_span_stop, metadata}, 500
    assert metadata.span == "runtime.stream"
    assert metadata["traceparent"] == "00-parent"
    assert metadata["x-request-id"] == "req_456"
    assert metadata.run_id == "run_1"
  end
end
