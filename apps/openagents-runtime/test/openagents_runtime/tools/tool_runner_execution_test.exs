defmodule OpenAgentsRuntime.Tools.ToolRunnerExecutionTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Spend.Authorizations
  alias OpenAgentsRuntime.Spend.Reservations
  alias OpenAgentsRuntime.Tools.ToolRunner
  alias OpenAgentsRuntime.Tools.ToolTasks

  test "persists streaming progress and succeeded terminal task records" do
    run_id = unique_run_id("tool_runner_success")
    insert_run(run_id)

    assert {:ok, %{"result" => "ok"}} =
             ToolRunner.run(
               fn report_progress ->
                 :ok = report_progress.(%{"phase" => "fetch"})
                 :ok = report_progress.(%{"phase" => "synthesize"})
                 %{"result" => "ok"}
               end,
               run_id: run_id,
               tool_call_id: "tool_call_success",
               tool_name: "web.search",
               input: %{"query" => "runtime"},
               timeout_ms: 2_000
             )

    task = ToolTasks.get_by_tool_call(run_id, "tool_call_success")
    assert task.state == "succeeded"
    assert %DateTime{} = task.running_at
    assert %DateTime{} = task.streaming_at
    assert %DateTime{} = task.succeeded_at
    assert task.output == %{"result" => "ok"}

    events = RunEvents.list_after(run_id, 0)
    event_types = Enum.map(events, & &1.event_type)
    assert "tool.call" in event_types
    assert "tool.progress" in event_types
    assert "tool.result" in event_types
    assert List.last(event_types) == "tool.result"
  end

  test "marks tool task timed_out when execution exceeds timeout budget" do
    run_id = unique_run_id("tool_runner_timeout")
    insert_run(run_id)

    assert {:error, :timeout} =
             ToolRunner.run(
               fn _report_progress ->
                 Process.sleep(300)
                 %{"result" => "late"}
               end,
               run_id: run_id,
               tool_call_id: "tool_call_timeout",
               tool_name: "long.job",
               timeout_ms: 50
             )

    task = ToolTasks.get_by_tool_call(run_id, "tool_call_timeout")
    assert task.state == "timed_out"
    assert task.error_class == "timeout"
    assert %DateTime{} = task.timed_out_at
  end

  test "cancel_run cancels in-flight tool tasks and persists canceled terminal state" do
    run_id = unique_run_id("tool_runner_cancel")
    insert_run(run_id)
    parent = self()

    task =
      Task.async(fn ->
        ToolRunner.run(
          fn _report_progress ->
            send(parent, :tool_started)
            Process.sleep(5_000)
            %{"result" => "never"}
          end,
          run_id: run_id,
          tool_call_id: "tool_call_cancel",
          tool_name: "stream.job",
          timeout_ms: 6_000
        )
      end)

    assert_receive :tool_started, 1_000
    assert :ok = ToolRunner.cancel_run(run_id)
    assert {:error, :canceled} = Task.await(task, 2_000)

    task_record = ToolTasks.get_by_tool_call(run_id, "tool_call_cancel")
    assert task_record.state == "canceled"
    assert task_record.error_class == "canceled"
    assert %DateTime{} = task_record.canceled_at
  end

  test "emits tool lifecycle telemetry for start and terminal outcomes" do
    run_id = unique_run_id("tool_runner_metrics")
    insert_run(run_id)
    handler_id = "tool-runner-telemetry-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        handler_id,
        [:openagents_runtime, :tool, :lifecycle],
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:tool_lifecycle, measurements, metadata})
        end,
        self()
      )

    on_exit(fn -> :telemetry.detach(handler_id) end)

    assert {:ok, %{"result" => "ok"}} =
             ToolRunner.run(
               fn ->
                 %{"result" => "ok"}
               end,
               run_id: run_id,
               tool_call_id: "tool_call_metrics",
               tool_name: "web.search",
               timeout_ms: 1_000
             )

    assert_receive {:tool_lifecycle, _measurements, %{phase: "run", result: "started"}}, 1_000

    assert_receive {:tool_lifecycle, measurements,
                    %{
                      phase: "terminal",
                      result: "succeeded",
                      state: "succeeded",
                      run_id: ^run_id,
                      tool_call_id: "tool_call_metrics"
                    }},
                   1_000

    assert measurements.count == 1
    assert measurements.duration_ms >= 0
  end

  test "rejects settlement-boundary tools missing idempotency metadata" do
    run_id = unique_run_id("tool_runner_settlement_missing_metadata")
    insert_run(run_id)
    authorization = insert_settlement_authorization!(run_id, 100)

    assert {:error, {:failed, "settlement_metadata_missing"}} =
             ToolRunner.run(
               fn ->
                 %{"provider_correlation_id" => "corr-missing", "result" => "ok"}
               end,
               run_id: run_id,
               tool_call_id: "tool_call_settlement_missing",
               tool_name: "payments.capture",
               settlement_boundary: true,
               authorization_id: authorization.authorization_id,
               amount_sats: 25
             )

    assert is_nil(
             Reservations.get(
               authorization.authorization_id,
               run_id,
               "tool_call_settlement_missing"
             )
           )

    task = ToolTasks.get_by_tool_call(run_id, "tool_call_settlement_missing")
    assert task.state == "failed"
    assert task.error_class == "settlement_metadata_missing"
  end

  test "commits settlement reservation with idempotency and correlation metadata" do
    run_id = unique_run_id("tool_runner_settlement_commit")
    insert_run(run_id)
    authorization = insert_settlement_authorization!(run_id, 250)

    assert {:ok, %{"provider_correlation_id" => "corr-commit-1", "result" => "ok"}} =
             ToolRunner.run(
               fn ->
                 %{"provider_correlation_id" => "corr-commit-1", "result" => "ok"}
               end,
               run_id: run_id,
               tool_call_id: "tool_call_settlement_commit",
               tool_name: "payments.capture",
               settlement_boundary: true,
               authorization_id: authorization.authorization_id,
               amount_sats: 40,
               settlement_retry_class: "dedupe_reconcile_required",
               provider_idempotency_key: "idem-commit-1"
             )

    reservation =
      Reservations.get(authorization.authorization_id, run_id, "tool_call_settlement_commit")

    assert reservation.state == "committed"
    assert reservation.provider_idempotency_key == "idem-commit-1"
    assert reservation.provider_correlation_id == "corr-commit-1"
    assert %DateTime{} = reservation.committed_at
  end

  test "unknown settlement outcomes enforce dedupe_reconcile_required on retry" do
    run_id = unique_run_id("tool_runner_settlement_reconcile")
    insert_run(run_id)
    authorization = insert_settlement_authorization!(run_id, 400)

    assert {:error, :timeout} =
             ToolRunner.run(
               fn _report_progress ->
                 Process.sleep(300)
                 %{"provider_correlation_id" => "corr-late", "result" => "late"}
               end,
               run_id: run_id,
               tool_call_id: "tool_call_settlement_reconcile",
               tool_name: "payments.capture",
               settlement_boundary: true,
               authorization_id: authorization.authorization_id,
               amount_sats: 50,
               settlement_retry_class: "dedupe_reconcile_required",
               provider_idempotency_key: "idem-reconcile-1",
               timeout_ms: 50
             )

    reservation =
      Reservations.get(authorization.authorization_id, run_id, "tool_call_settlement_reconcile")

    assert reservation.state == "reconcile_required"

    assert {:error, {:failed, "dedupe_reconcile_required"}} =
             ToolRunner.run(
               fn ->
                 %{"provider_correlation_id" => "corr-retry", "result" => "retry"}
               end,
               run_id: run_id,
               tool_call_id: "tool_call_settlement_reconcile",
               tool_name: "payments.capture",
               settlement_boundary: true,
               authorization_id: authorization.authorization_id,
               amount_sats: 50,
               settlement_retry_class: "dedupe_reconcile_required",
               provider_idempotency_key: "idem-reconcile-1"
             )
  end

  defp insert_run(run_id) do
    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: "thread_#{run_id}",
      status: "running",
      owner_user_id: 51,
      latest_seq: 0
    })
  end

  defp unique_run_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end

  defp insert_settlement_authorization!(run_id, max_total_sats) do
    run = Repo.get!(Run, run_id)

    payload = %{
      owner_user_id: run.owner_user_id,
      run_id: run_id,
      mode: "delegated_budget",
      max_total_sats: max_total_sats,
      spent_sats: 0,
      reserved_sats: 0,
      constraints: %{},
      metadata: %{}
    }

    case Authorizations.create(payload) do
      {:ok, authorization} -> authorization
      {:error, changeset} -> flunk("authorization insert failed: #{inspect(changeset.errors)}")
    end
  end
end
