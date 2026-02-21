defmodule OpenAgentsRuntime.Sync.TopicPolicy do
  @moduledoc """
  Topic-class policy registry for Khala retention, compaction, and snapshot behavior.

  This module is the canonical runtime source for:

  - per-topic retention windows,
  - compaction strategy class metadata,
  - QoS tier and replay budget policy,
  - snapshot bootstrap metadata surfaced in stale-cursor flows.
  """

  @default_retention_seconds 86_400
  @default_qos_tier "warm"
  @default_replay_budget_events 10_000
  @snapshot_format "openagents.sync.snapshot.v1"

  @default_topic_policies %{
    "runtime.run_summaries" => %{
      "topic_class" => "durable_summary",
      "qos_tier" => "warm",
      "replay_budget_events" => 20_000,
      "retention_seconds" => 604_800,
      "compaction_mode" => "tail_prune_with_snapshot_rehydrate",
      "snapshot" => %{
        "enabled" => true,
        "format" => @snapshot_format,
        "schema_version" => 1,
        "cadence_seconds" => 300,
        "source_table" => "runtime.sync_run_summaries"
      }
    },
    "runtime.codex_worker_summaries" => %{
      "topic_class" => "durable_summary",
      "qos_tier" => "warm",
      "replay_budget_events" => 10_000,
      "retention_seconds" => 259_200,
      "compaction_mode" => "tail_prune_with_snapshot_rehydrate",
      "snapshot" => %{
        "enabled" => true,
        "format" => @snapshot_format,
        "schema_version" => 1,
        "cadence_seconds" => 120,
        "source_table" => "runtime.sync_codex_worker_summaries"
      }
    },
    "runtime.codex_worker_events" => %{
      "topic_class" => "high_churn_events",
      "qos_tier" => "hot",
      "replay_budget_events" => 3_000,
      "retention_seconds" => 86_400,
      "compaction_mode" => "tail_prune_without_snapshot",
      "snapshot" => %{"enabled" => false}
    },
    "runtime.notifications" => %{
      "topic_class" => "ephemeral_notifications",
      "qos_tier" => "cold",
      "replay_budget_events" => 500,
      "retention_seconds" => 43_200,
      "compaction_mode" => "tail_prune_without_snapshot",
      "snapshot" => %{"enabled" => false}
    }
  }

  @type topic_policies :: %{optional(String.t()) => map()}

  @spec topic_policies() :: topic_policies()
  def topic_policies do
    configured = Application.get_env(:openagents_runtime, :khala_sync_topic_policies, %{})
    topic_policies(configured)
  end

  @spec topic_policies(map()) :: topic_policies()
  def topic_policies(configured) when is_map(configured) do
    normalized =
      configured
      |> normalize_topic_policy_map()

    merged_defaults =
      Map.merge(@default_topic_policies, normalized, fn _topic, default_policy, override_policy ->
        merge_policy(default_policy, override_policy)
      end)

    extra_topics =
      normalized
      |> Enum.reject(fn {topic, _policy} -> Map.has_key?(@default_topic_policies, topic) end)
      |> Map.new()

    Map.merge(merged_defaults, extra_topics)
  end

  @spec known_topics() :: [String.t()]
  def known_topics do
    topic_policies()
    |> Map.keys()
    |> Enum.sort()
  end

  @spec known_topics(topic_policies()) :: [String.t()]
  def known_topics(policies) when is_map(policies) do
    policies
    |> Map.keys()
    |> Enum.sort()
  end

  @spec retention_seconds(String.t(), topic_policies(), pos_integer()) :: pos_integer()
  def retention_seconds(topic, policies, fallback \\ @default_retention_seconds)
      when is_binary(topic) and is_map(policies) and is_integer(fallback) and fallback > 0 do
    policies
    |> Map.get(topic, %{})
    |> Map.get("retention_seconds", fallback)
    |> normalize_positive_integer(fallback)
  end

  @spec topic_class(String.t(), topic_policies()) :: String.t()
  def topic_class(topic, policies) when is_binary(topic) and is_map(policies) do
    policies
    |> Map.get(topic, %{})
    |> Map.get("topic_class", "unspecified")
    |> normalize_string("unspecified")
  end

  @spec compaction_mode(String.t(), topic_policies()) :: String.t()
  def compaction_mode(topic, policies) when is_binary(topic) and is_map(policies) do
    policies
    |> Map.get(topic, %{})
    |> Map.get("compaction_mode", "tail_prune_with_snapshot_rehydrate")
    |> normalize_string("tail_prune_with_snapshot_rehydrate")
  end

  @spec qos_tier(String.t(), topic_policies()) :: String.t()
  def qos_tier(topic, policies) when is_binary(topic) and is_map(policies) do
    policies
    |> Map.get(topic, %{})
    |> Map.get("qos_tier", @default_qos_tier)
    |> normalize_string(@default_qos_tier)
  end

  @spec replay_budget_events(String.t(), topic_policies(), pos_integer()) :: pos_integer()
  def replay_budget_events(topic, policies, fallback \\ @default_replay_budget_events)
      when is_binary(topic) and is_map(policies) and is_integer(fallback) and fallback > 0 do
    policies
    |> Map.get(topic, %{})
    |> Map.get("replay_budget_events", fallback)
    |> normalize_positive_integer(fallback)
  end

  @spec snapshot_metadata(String.t(), topic_policies()) :: map() | nil
  def snapshot_metadata(topic, policies \\ topic_policies())
      when is_binary(topic) and is_map(policies) do
    with %{} = policy <- Map.get(policies, topic),
         %{} = snapshot <- Map.get(policy, "snapshot"),
         true <- snapshot_enabled?(snapshot) do
      snapshot
      |> Map.drop(["enabled"])
      |> Map.put_new("format", @snapshot_format)
      |> Map.put_new("schema_version", 1)
      |> Map.put("topic", topic)
    else
      _other -> nil
    end
  end

  defp merge_policy(default_policy, override_policy)
       when is_map(default_policy) and is_map(override_policy) do
    merged = Map.merge(default_policy, override_policy)

    snapshot =
      case {Map.get(default_policy, "snapshot"), Map.get(override_policy, "snapshot")} do
        {%{} = default_snapshot, %{} = override_snapshot} ->
          Map.merge(default_snapshot, normalize_snapshot(override_snapshot))

        {_default_snapshot, %{} = override_snapshot} ->
          normalize_snapshot(override_snapshot)

        {%{} = default_snapshot, _other} ->
          default_snapshot

        _other ->
          %{"enabled" => false}
      end

    merged
    |> Map.put("snapshot", snapshot)
    |> normalize_policy()
  end

  defp normalize_topic_policy_map(policies) when is_map(policies) do
    policies
    |> Enum.reduce(%{}, fn {topic, policy}, acc ->
      topic_key = normalize_string(topic, nil)

      if is_binary(topic_key) and topic_key != "" and is_map(policy) do
        Map.put(acc, topic_key, normalize_policy(policy))
      else
        acc
      end
    end)
  end

  defp normalize_policy(policy) when is_map(policy) do
    topic_class = normalize_string(policy["topic_class"] || policy[:topic_class], "unspecified")
    qos_tier = normalize_string(policy["qos_tier"] || policy[:qos_tier], @default_qos_tier)

    replay_budget_events =
      normalize_positive_integer(
        policy["replay_budget_events"] || policy[:replay_budget_events],
        @default_replay_budget_events
      )

    retention_seconds =
      normalize_positive_integer(
        policy["retention_seconds"] || policy[:retention_seconds],
        @default_retention_seconds
      )

    compaction_mode =
      normalize_string(
        policy["compaction_mode"] || policy[:compaction_mode],
        "tail_prune_with_snapshot_rehydrate"
      )

    snapshot =
      policy
      |> Map.get("snapshot", Map.get(policy, :snapshot, %{"enabled" => false}))
      |> case do
        value when is_map(value) -> normalize_snapshot(value)
        _other -> %{"enabled" => false}
      end

    %{
      "topic_class" => topic_class,
      "qos_tier" => qos_tier,
      "replay_budget_events" => replay_budget_events,
      "retention_seconds" => retention_seconds,
      "compaction_mode" => compaction_mode,
      "snapshot" => snapshot
    }
  end

  defp normalize_snapshot(snapshot) when is_map(snapshot) do
    enabled = snapshot_enabled?(snapshot)

    %{
      "enabled" => enabled,
      "format" => normalize_string(snapshot["format"] || snapshot[:format], @snapshot_format),
      "schema_version" =>
        normalize_positive_integer(snapshot["schema_version"] || snapshot[:schema_version], 1),
      "cadence_seconds" =>
        normalize_positive_integer(snapshot["cadence_seconds"] || snapshot[:cadence_seconds], 300),
      "source_table" =>
        normalize_string(
          snapshot["source_table"] || snapshot[:source_table],
          "runtime.sync_unknown"
        )
    }
  end

  defp snapshot_enabled?(snapshot) when is_map(snapshot) do
    case snapshot["enabled"] || snapshot[:enabled] do
      value when value in [true, "true", "TRUE", "1", 1, true] -> true
      _other -> false
    end
  end

  defp snapshot_enabled?(_snapshot), do: false

  defp normalize_positive_integer(value, _fallback) when is_integer(value) and value > 0,
    do: value

  defp normalize_positive_integer(value, fallback) when is_binary(value) do
    case Integer.parse(String.trim(value)) do
      {parsed, ""} when parsed > 0 -> parsed
      _other -> fallback
    end
  end

  defp normalize_positive_integer(_value, fallback), do: fallback

  defp normalize_string(value, fallback) when is_binary(value) do
    case String.trim(value) do
      "" -> fallback
      trimmed -> trimmed
    end
  end

  defp normalize_string(nil, fallback), do: fallback

  defp normalize_string(value, fallback) when is_atom(value) do
    value |> Atom.to_string() |> normalize_string(fallback)
  end

  defp normalize_string(_value, fallback), do: fallback
end
