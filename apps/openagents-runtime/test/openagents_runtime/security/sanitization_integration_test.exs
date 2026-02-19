defmodule OpenAgentsRuntime.Security.SanitizationIntegrationTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.DS.Traces
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Telemetry.Events
  alias OpenAgentsRuntime.Tools.ToolRunner
  alias OpenAgentsRuntime.Tools.ToolTask
  alias OpenAgentsRuntime.Tools.ToolTasks

  test "run events persist sanitized payloads" do
    run_id = unique_run_id("sanitize_events")
    insert_run(run_id)

    assert {:ok, _event} =
             RunEvents.append_event(run_id, "run.delta", %{
               "authorization" => "Bearer secret",
               "api_key" => "sk-live-abc",
               "email" => "user@example.com",
               "safe" => "ok"
             })

    [event] = RunEvents.list_after(run_id, 0)
    payload = event.payload

    assert payload["authorization"] == "[REDACTED]"
    assert payload["api_key"] == "[REDACTED]"
    assert payload["email"] == "[REDACTED_EMAIL]"
    assert payload["safe"] == "ok"
  end

  test "tool runner sanitizes tool task and event payload surfaces" do
    run_id = unique_run_id("sanitize_tool")
    tool_call_id = "tool_sanitize_1"
    insert_run(run_id)

    assert {:ok, %{"result" => "ok"}} =
             ToolRunner.run(
               fn ->
                 %{
                   "result" => "ok",
                   "password" => "supersecret",
                   "contact" => "person@example.com"
                 }
               end,
               run_id: run_id,
               tool_call_id: tool_call_id,
               tool_name: "web.search",
               input: %{
                 "query" => "hello",
                 "api_key" => "sk-live-123",
                 "authorization" => "Bearer abc.def.ghi"
               },
               timeout_ms: 1_000
             )

    %ToolTask{} = task = ToolTasks.get_by_tool_call(run_id, tool_call_id)
    assert task.input["api_key"] == "[REDACTED]"
    assert task.input["authorization"] == "[REDACTED]"
    assert task.output["password"] == "[REDACTED]"
    assert task.output["contact"] == "[REDACTED_EMAIL]"

    events = RunEvents.list_after(run_id, 0)

    call_event = Enum.find(events, &(&1.event_type == "tool.call"))
    result_event = Enum.find(events, &(&1.event_type == "tool.result"))

    assert call_event.payload["input"]["api_key"] == "[REDACTED]"
    assert call_event.payload["input"]["authorization"] == "[REDACTED]"
    assert result_event.payload["output"]["password"] == "[REDACTED]"
    assert result_event.payload["output"]["contact"] == "[REDACTED_EMAIL]"
  end

  test "trace capture sanitizes stored payloads" do
    trace =
      Traces.capture("run_trace_sanitize", "@openagents/test", %{
        "headers" => %{"authorization" => "Bearer trace-secret"},
        "api_key" => "sk-live-xyz",
        "email" => "trace@example.com",
        "safe" => "ok"
      })

    payload = trace["payload"]
    assert payload["headers"]["authorization"] == "[REDACTED]"
    assert payload["api_key"] == "[REDACTED]"
    assert payload["email"] == "[REDACTED_EMAIL]"
    assert payload["safe"] == "ok"
  end

  test "telemetry emission sanitizes metadata while preserving operational keys" do
    handler_id = "sanitize-telemetry-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        handler_id,
        [:openagents_runtime, :sanitizer, :probe],
        fn _event_name, _measurements, metadata, test_pid ->
          send(test_pid, {:metadata, metadata})
        end,
        self()
      )

    on_exit(fn -> :telemetry.detach(handler_id) end)

    Events.emit(
      [:openagents_runtime, :sanitizer, :probe],
      %{count: 1},
      %{
        run_id: "run_probe",
        authorization: "Bearer foo.bar.baz",
        api_key: "sk-live-999",
        email: "probe@example.com"
      }
    )

    assert_receive {:metadata, metadata}, 1_000
    assert metadata.run_id == "run_probe"
    assert metadata.authorization == "[REDACTED]"
    assert metadata.api_key == "[REDACTED]"
    assert metadata.email == "[REDACTED_EMAIL]"
  end

  defp insert_run(run_id) do
    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: "thread_#{run_id}",
      status: "running",
      owner_user_id: 5,
      latest_seq: 0
    })
  end

  defp unique_run_id(prefix) do
    "#{prefix}_#{System.unique_integer([:positive])}"
  end
end
