defmodule OpenAgentsRuntime.Tools.Policy.OpenClawToolPolicyPipelineTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Policy.OpenClawToolPolicyPipeline

  test "build_default_tool_policy_pipeline_steps preserves upstream layer order and labels" do
    steps =
      OpenClawToolPolicyPipeline.build_default_tool_policy_pipeline_steps(%{
        profile: "coding",
        provider_profile: "anthropic",
        agent_id: "agent_42"
      })

    labels = Enum.map(steps, & &1.label)

    assert labels == [
             "tools.profile (coding)",
             "tools.byProvider.profile (anthropic)",
             "tools.allow",
             "tools.byProvider.allow",
             "agents.agent_42.tools.allow",
             "agents.agent_42.tools.byProvider.allow",
             "group tools.allow"
           ]

    assert Enum.all?(steps, &(&1.strip_plugin_only_allowlist == true))
  end

  test "apply_tool_policy_pipeline emits deterministic warnings for unknown allowlist entries" do
    tools = [
      %{"name" => "read"},
      %{"name" => "calendar.send", "plugin_id" => "plugin.calendar"}
    ]

    steps = [
      %{
        "policy" => %{"allow" => ["group:unknown"]},
        "label" => "tools.allow",
        "stripPluginOnlyAllowlist" => true
      }
    ]

    warn = fn message -> send(self(), {:warn, message}) end

    filtered =
      OpenClawToolPolicyPipeline.apply_tool_policy_pipeline(%{
        tools: tools,
        steps: steps,
        tool_meta: fn tool ->
          case tool do
            %{"plugin_id" => plugin_id} when is_binary(plugin_id) -> %{plugin_id: plugin_id}
            _ -> nil
          end
        end,
        warn: warn
      })

    assert Enum.map(filtered, & &1["name"]) == ["read", "calendar.send"]
    assert_receive {:warn, warning}

    assert warning ==
             "tools: tools.allow allowlist contains unknown entries (group:unknown). " <>
               "Ignoring allowlist so core tools remain available. " <>
               "Use tools.alsoAllow for additive plugin tool enablement."
  end

  test "filter_tools_by_policy supports wildcard matching and apply_patch alias rule" do
    tools = [
      %{name: "read"},
      %{name: "apply_patch"},
      %{name: "exec"},
      %{name: "gateway"}
    ]

    filtered =
      OpenClawToolPolicyPipeline.filter_tools_by_policy(tools, %{
        allow: ["re*", "exec"],
        deny: ["gateway"]
      })

    assert Enum.map(filtered, & &1.name) == ["read", "apply_patch", "exec"]
  end
end
