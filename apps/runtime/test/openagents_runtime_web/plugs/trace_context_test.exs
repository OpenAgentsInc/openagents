defmodule OpenAgentsRuntimeWeb.Plugs.TraceContextTest do
  use ExUnit.Case, async: false

  alias OpenAgentsRuntime.Telemetry.Tracing
  alias OpenAgentsRuntimeWeb.Plugs.TraceContext

  setup do
    Process.delete(:openagents_runtime_trace_context)
    :ok
  end

  test "stores trace context in conn private and process context" do
    conn =
      Plug.Test.conn(:get, "/internal/v1/health")
      |> Plug.Conn.put_req_header("traceparent", "00-aaa-bbb-01")
      |> Plug.Conn.put_req_header("x-request-id", "req_ctx")
      |> TraceContext.call([])

    assert conn.private[:trace_context]["traceparent"] == "00-aaa-bbb-01"
    assert Tracing.current()["traceparent"] == "00-aaa-bbb-01"
    assert Tracing.current()["x-request-id"] == "req_ctx"
  end

  test "backfills response x-request-id header when available" do
    conn =
      Plug.Test.conn(:get, "/internal/v1/health")
      |> Plug.Conn.put_resp_header("x-request-id", "req_from_conn")
      |> TraceContext.call([])
      |> Plug.Conn.send_resp(200, "ok")

    assert ["req_from_conn"] = Plug.Conn.get_resp_header(conn, "x-request-id")
    assert Tracing.current()["x-request-id"] == "req_from_conn"
  end
end
