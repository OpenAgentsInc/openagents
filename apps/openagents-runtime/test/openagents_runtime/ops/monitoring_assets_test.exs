defmodule OpenAgentsRuntime.Ops.MonitoringAssetsTest do
  use ExUnit.Case, async: true

  @project_root Path.expand("../../..", __DIR__)
  @dashboard_path Path.join(
                    @project_root,
                    "deploy/monitoring/grafana/openagents-runtime-ops-dashboard.json"
                  )
  @alerts_path Path.join(
                 @project_root,
                 "deploy/monitoring/prometheus/openagents-runtime-alert-rules.yaml"
               )

  test "dashboard includes required runtime operations panels" do
    dashboard = @dashboard_path |> File.read!() |> Jason.decode!()
    panels = Map.get(dashboard, "panels", [])
    titles = Enum.map(panels, &Map.get(&1, "title"))

    required_titles = [
      "Executor p95 Run Duration (ms)",
      "Runtime 5xx Rate",
      "Stream Completion Ratio ([DONE]/session)",
      "Lease Steal Rate",
      "Tool Terminal Failures",
      "Provider Circuit Breaker Open",
      "Spend/Policy Denial Ratio",
      "Khala Projection Writes/s",
      "Khala Projection Lag p95 (events)",
      "Khala Projection Write Failure Ratio",
      "Khala Projection Drift Incidents (10m)",
      "Khala Projection Replay Errors (15m)",
      "Khala Token Mint Failure Ratio"
    ]

    Enum.each(required_titles, fn title ->
      assert title in titles
    end)

    assert Enum.all?(panels, fn panel ->
             panel
             |> Map.get("targets", [])
             |> Enum.all?(fn target ->
               expr = Map.get(target, "expr", "")
               is_binary(expr) and expr != ""
             end)
           end)
  end

  test "alert rules include required SLO and safety guards" do
    {:ok, alert_rules} = YamlElixir.read_from_file(@alerts_path)

    rules =
      alert_rules
      |> get_in(["spec", "groups"])
      |> List.first()
      |> Map.get("rules", [])

    alerts = Enum.map(rules, &Map.get(&1, "alert"))

    required_alerts = [
      "OpenAgentsRuntimeExecutorLatencyP95High",
      "OpenAgentsRuntimeHttp5xxRateHigh",
      "OpenAgentsRuntimeStreamDoneRatioLow",
      "OpenAgentsRuntimeLeaseStealRateHigh",
      "OpenAgentsRuntimeToolFailureSpike",
      "OpenAgentsRuntimeCircuitBreakerOpen",
      "OpenAgentsRuntimePolicyDenialAnomaly",
      "OpenAgentsRuntimeKhalaProjectionLagP95High",
      "OpenAgentsRuntimeKhalaProjectionWriteFailureRatioHigh",
      "OpenAgentsRuntimeKhalaProjectionDriftIncidentsHigh",
      "OpenAgentsRuntimeKhalaProjectionReplayFailures",
      "OpenAgentsKhalaTokenMintFailureRatioHigh"
    ]

    Enum.each(required_alerts, fn alert ->
      assert alert in alerts
    end)

    assert Enum.all?(rules, fn rule ->
             runbook = get_in(rule, ["annotations", "runbook"])
             is_binary(runbook) and runbook != ""
           end)
  end
end
