defmodule OpenAgentsRuntime.Tools.ToolRunnerTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Telemetry.Tracing
  alias OpenAgentsRuntime.Tools.ToolRunner

  test "propagates trace context into supervised tool task" do
    :ok = Tracing.put_current(%{"traceparent" => "00-tool", "x-request-id" => "req_tool"})

    assert {:ok, context} =
             ToolRunner.run(fn ->
               Tracing.current()
             end)

    assert context["traceparent"] == "00-tool"
    assert context["x-request-id"] == "req_tool"
  end
end
