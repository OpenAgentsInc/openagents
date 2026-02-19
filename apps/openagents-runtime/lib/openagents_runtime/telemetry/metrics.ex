defmodule OpenAgentsRuntime.Telemetry.Metrics do
  @moduledoc """
  Runtime metrics declaration boundary with explicit cardinality guardrails.

  High-cardinality identifiers remain in telemetry metadata for logs/traces, but
  metric tags are constrained to bounded taxonomies.
  """

  import Telemetry.Metrics

  @metric_tag_allowlist %{
    executor_frame_processed: [:frame_type, :duplicate],
    executor_terminal: [:status, :reason_class],
    executor_run_started: [:status],
    executor_run_once: [:result, :reason_class],
    stream_emit: [:event_type, :outcome],
    stream_session: [:outcome],
    run_events_notify: [],
    agent_process_stats: [:event],
    tool_lifecycle: [:phase, :result, :state, :error_class],
    provider_breaker_state: [:provider, :state, :event],
    lease_operation: [:action, :result],
    janitor_cycle: [],
    janitor_resumed: [],
    janitor_failed: [:reason_class],
    policy_decision: [:decision, :authorization_mode, :settlement_boundary],
    parity_failure: [:class, :reason_class, :component, :outcome]
  }

  @high_cardinality_tags [
    :run_id,
    :thread_id,
    :frame_id,
    :tool_call_id,
    :authorization_id,
    :lease_owner,
    :traceparent,
    :tracestate,
    :x_request_id,
    :seq,
    :cursor,
    :user_id,
    :guest_scope
  ]

  @spec default_prefix() :: String.t()
  def default_prefix, do: "openagents_runtime"

  @spec metric_tag_allowlist() :: %{atom() => [atom()]}
  def metric_tag_allowlist, do: @metric_tag_allowlist

  @spec high_cardinality_tags() :: [atom()]
  def high_cardinality_tags, do: @high_cardinality_tags

  @spec metrics() :: [Telemetry.Metrics.t()]
  def metrics do
    [
      counter("openagents_runtime.executor.frame_processed.count",
        tags: tags_for(:executor_frame_processed)
      ),
      counter("openagents_runtime.executor.terminal.count", tags: tags_for(:executor_terminal)),
      counter("openagents_runtime.executor.run_started.count",
        tags: tags_for(:executor_run_started)
      ),
      counter("openagents_runtime.executor.run_once.count", tags: tags_for(:executor_run_once)),
      summary("openagents_runtime.executor.run_once.duration_ms",
        tags: tags_for(:executor_run_once)
      ),
      counter("openagents_runtime.stream.emit.count", tags: tags_for(:stream_emit)),
      summary("openagents_runtime.stream.emit.frames", tags: tags_for(:stream_emit)),
      counter("openagents_runtime.stream.session.count", tags: tags_for(:stream_session)),
      summary("openagents_runtime.stream.session.duration_ms", tags: tags_for(:stream_session)),
      summary("openagents_runtime.stream.session.emitted_events",
        tags: tags_for(:stream_session)
      ),
      summary("openagents_runtime.stream.session.emitted_chunks",
        tags: tags_for(:stream_session)
      ),
      summary("openagents_runtime.stream.session.wakeups", tags: tags_for(:stream_session)),
      summary("openagents_runtime.stream.session.polls", tags: tags_for(:stream_session)),
      counter("openagents_runtime.run_events.notify.count", tags: tags_for(:run_events_notify)),
      summary("openagents_runtime.agent_process.stats.message_queue_len",
        tags: tags_for(:agent_process_stats)
      ),
      summary("openagents_runtime.agent_process.stats.reductions",
        tags: tags_for(:agent_process_stats)
      ),
      counter("openagents_runtime.tool.lifecycle.count", tags: tags_for(:tool_lifecycle)),
      summary("openagents_runtime.tool.lifecycle.duration_ms", tags: tags_for(:tool_lifecycle)),
      last_value("openagents_runtime.provider.breaker.state",
        tags: tags_for(:provider_breaker_state)
      ),
      counter("openagents_runtime.lease.operation.count", tags: tags_for(:lease_operation)),
      counter("openagents_runtime.janitor.cycle.count", tags: tags_for(:janitor_cycle)),
      summary("openagents_runtime.janitor.cycle.scanned", tags: tags_for(:janitor_cycle)),
      summary("openagents_runtime.janitor.cycle.resumed", tags: tags_for(:janitor_cycle)),
      summary("openagents_runtime.janitor.cycle.failed", tags: tags_for(:janitor_cycle)),
      summary("openagents_runtime.janitor.cycle.skipped", tags: tags_for(:janitor_cycle)),
      counter("openagents_runtime.janitor.resumed.count", tags: tags_for(:janitor_resumed)),
      counter("openagents_runtime.janitor.failed.count", tags: tags_for(:janitor_failed)),
      counter("openagents_runtime.policy.decision.count", tags: tags_for(:policy_decision)),
      summary("openagents_runtime.policy.decision.spent_sats", tags: tags_for(:policy_decision)),
      summary("openagents_runtime.policy.decision.reserved_sats",
        tags: tags_for(:policy_decision)
      ),
      summary("openagents_runtime.policy.decision.remaining_sats",
        tags: tags_for(:policy_decision)
      ),
      counter("openagents_runtime.parity.failure.count", tags: tags_for(:parity_failure))
    ]
  end

  defp tags_for(metric_key), do: Map.fetch!(@metric_tag_allowlist, metric_key)
end
