defmodule OpenAgentsRuntime.Telemetry.MetricsTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Telemetry.Metrics

  test "runtime metrics never tag high-cardinality identifiers" do
    forbidden = MapSet.new(Metrics.high_cardinality_tags())

    tagged_metrics =
      Metrics.metrics()
      |> Enum.flat_map(fn metric ->
        metric_tags = Map.get(metric, :tags, [])
        Enum.map(metric_tags, fn tag -> {metric.name, tag} end)
      end)

    violating =
      Enum.filter(tagged_metrics, fn {_metric_name, tag} ->
        MapSet.member?(forbidden, tag)
      end)

    assert violating == []
  end

  test "metric allowlist covers all runtime telemetry families" do
    allowlist = Metrics.metric_tag_allowlist()

    assert Map.has_key?(allowlist, :executor_frame_processed)
    assert Map.has_key?(allowlist, :stream_session)
    assert Map.has_key?(allowlist, :tool_lifecycle)
    assert Map.has_key?(allowlist, :provider_breaker_state)
    assert Map.has_key?(allowlist, :lease_operation)
    assert Map.has_key?(allowlist, :janitor_cycle)
    assert Map.has_key?(allowlist, :policy_decision)
  end
end
