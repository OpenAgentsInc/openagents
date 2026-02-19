defmodule OpenAgentsRuntime.Tools.ToolRunnerTest do
  use OpenAgentsRuntime.DataCase, async: false

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

  test "cancel_run best-effort cancels in-flight run scoped tasks" do
    parent = self()

    task =
      Task.async(fn ->
        ToolRunner.run(
          fn ->
            send(parent, :tool_started)
            Process.sleep(5_000)
            :done
          end,
          run_id: "run_cancel_test",
          timeout_ms: 6_000
        )
      end)

    assert_receive :tool_started, 1_000
    assert :ok = ToolRunner.cancel_run("run_cancel_test")
    assert {:error, :canceled} = Task.await(task, 2_000)
  end
end
