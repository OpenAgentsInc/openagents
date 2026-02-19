defmodule OpenAgentsRuntime.Tools.Policy.OpenClawToolPolicyPipeline do
  @moduledoc """
  OpenClaw-equivalent layered tool-policy pipeline.

  Upstream references:
  - /Users/christopherdavid/code/openclaw/src/agents/tool-policy-pipeline.ts
  - /Users/christopherdavid/code/openclaw/src/agents/pi-tools.policy.ts
  - commit 8e1f25631b220f139e79003caecabd11b7e1e748
  """

  alias OpenAgentsRuntime.Tools.Policy.OpenClawToolPolicy

  @type tool_policy_like :: %{
          optional(:allow) => [String.t()],
          optional(:deny) => [String.t()]
        }

  @type tool_policy_pipeline_step :: %{
          policy: nil | tool_policy_like(),
          label: String.t(),
          strip_plugin_only_allowlist: boolean()
        }

  @type tool_meta :: %{
          optional(:plugin_id) => String.t(),
          optional(:pluginId) => String.t()
        }

  @spec build_default_tool_policy_pipeline_steps(map()) :: [tool_policy_pipeline_step()]
  def build_default_tool_policy_pipeline_steps(params) when is_map(params) do
    agent_id = optional_trim(params, [:agent_id, "agent_id", :agentId, "agentId"])
    profile = optional_trim(params, [:profile, "profile"])

    provider_profile =
      optional_trim(params, [
        :provider_profile,
        "provider_profile",
        :providerProfile,
        "providerProfile"
      ])

    [
      %{
        policy:
          policy_from(params, [:profile_policy, "profile_policy", :profilePolicy, "profilePolicy"]),
        label: if(profile, do: "tools.profile (#{profile})", else: "tools.profile"),
        strip_plugin_only_allowlist: true
      },
      %{
        policy:
          policy_from(params, [
            :provider_profile_policy,
            "provider_profile_policy",
            :providerProfilePolicy,
            "providerProfilePolicy"
          ]),
        label:
          if(
            provider_profile,
            do: "tools.byProvider.profile (#{provider_profile})",
            else: "tools.byProvider.profile"
          ),
        strip_plugin_only_allowlist: true
      },
      %{
        policy:
          policy_from(params, [:global_policy, "global_policy", :globalPolicy, "globalPolicy"]),
        label: "tools.allow",
        strip_plugin_only_allowlist: true
      },
      %{
        policy:
          policy_from(params, [
            :global_provider_policy,
            "global_provider_policy",
            :globalProviderPolicy,
            "globalProviderPolicy"
          ]),
        label: "tools.byProvider.allow",
        strip_plugin_only_allowlist: true
      },
      %{
        policy: policy_from(params, [:agent_policy, "agent_policy", :agentPolicy, "agentPolicy"]),
        label: if(agent_id, do: "agents.#{agent_id}.tools.allow", else: "agent tools.allow"),
        strip_plugin_only_allowlist: true
      },
      %{
        policy:
          policy_from(params, [
            :agent_provider_policy,
            "agent_provider_policy",
            :agentProviderPolicy,
            "agentProviderPolicy"
          ]),
        label:
          if(
            agent_id,
            do: "agents.#{agent_id}.tools.byProvider.allow",
            else: "agent tools.byProvider.allow"
          ),
        strip_plugin_only_allowlist: true
      },
      %{
        policy: policy_from(params, [:group_policy, "group_policy", :groupPolicy, "groupPolicy"]),
        label: "group tools.allow",
        strip_plugin_only_allowlist: true
      }
    ]
  end

  @doc """
  Applies layered policy filtering across tool entries.

  Params:
  - `:tools` list of tool maps/structs with at least a `name` field.
  - `:tool_meta` function `(tool -> %{plugin_id: ...} | %{pluginId: ...} | nil)`.
  - `:warn` function `(String.t() -> any())` for deterministic policy warnings.
  - `:steps` ordered policy layers to apply.
  """
  @spec apply_tool_policy_pipeline(%{
          required(:tools) => [map()],
          required(:steps) => [tool_policy_pipeline_step()],
          optional(:tool_meta) => (map() -> nil | tool_meta()),
          optional(:warn) => (String.t() -> any())
        }) :: [map()]
  def apply_tool_policy_pipeline(params) when is_map(params) do
    tools = Map.get(params, :tools, Map.get(params, "tools", []))
    steps = Map.get(params, :steps, Map.get(params, "steps", []))
    tool_meta = Map.get(params, :tool_meta, Map.get(params, "tool_meta", &default_tool_meta/1))
    warn = Map.get(params, :warn, Map.get(params, "warn", fn _message -> :ok end))

    core_tool_names =
      tools
      |> Enum.filter(&(normalize_meta(tool_meta.(&1)) == nil))
      |> Enum.map(&tool_name/1)
      |> Enum.map(&OpenClawToolPolicy.normalize_tool_name/1)
      |> Enum.reject(&(&1 == ""))
      |> MapSet.new()

    plugin_groups =
      tools
      |> Enum.map(fn tool ->
        case normalize_meta(tool_meta.(tool)) do
          nil ->
            %{name: tool_name(tool)}

          plugin_id ->
            %{name: tool_name(tool), plugin_id: plugin_id}
        end
      end)
      |> OpenClawToolPolicy.build_plugin_tool_groups()

    Enum.reduce(steps, tools, fn step, filtered ->
      policy =
        step
        |> Map.get(:policy, Map.get(step, "policy"))
        |> normalize_policy()

      if is_nil(policy) do
        filtered
      else
        step_label = Map.get(step, :label, Map.get(step, "label", "tools.allow"))

        strip_allowlist? =
          Map.get(
            step,
            :strip_plugin_only_allowlist,
            Map.get(
              step,
              "strip_plugin_only_allowlist",
              Map.get(step, "stripPluginOnlyAllowlist", false)
            )
          )

        policy =
          if strip_allowlist? do
            resolved =
              OpenClawToolPolicy.strip_plugin_only_allowlist(
                policy,
                plugin_groups,
                core_tool_names
              )

            if resolved.unknown_allowlist != [] do
              entries = Enum.join(resolved.unknown_allowlist, ", ")

              suffix =
                if resolved.stripped_allowlist do
                  "Ignoring allowlist so core tools remain available. Use tools.alsoAllow for additive plugin tool enablement."
                else
                  "These entries won't match any tool unless the plugin is enabled."
                end

              warn.(
                "tools: #{step_label} allowlist contains unknown entries (#{entries}). #{suffix}"
              )
            end

            resolved.policy
          else
            policy
          end

        expanded = OpenClawToolPolicy.expand_policy_with_plugin_groups(policy, plugin_groups)

        if expanded do
          filter_tools_by_policy(filtered, expanded)
        else
          filtered
        end
      end
    end)
  end

  @spec filter_tools_by_policy([map()], tool_policy_like() | nil) :: [map()]
  def filter_tools_by_policy(tools, nil), do: tools

  def filter_tools_by_policy(tools, policy) when is_list(tools) and is_map(policy) do
    deny_patterns =
      policy
      |> Map.get(:deny, Map.get(policy, "deny", []))
      |> OpenClawToolPolicy.expand_tool_groups()
      |> compile_glob_patterns()

    allow_patterns =
      policy
      |> Map.get(:allow, Map.get(policy, "allow", []))
      |> OpenClawToolPolicy.expand_tool_groups()
      |> compile_glob_patterns()

    Enum.filter(tools, fn tool ->
      normalized_name =
        tool
        |> tool_name()
        |> OpenClawToolPolicy.normalize_tool_name()

      cond do
        matches_any_glob_pattern?(normalized_name, deny_patterns) ->
          false

        allow_patterns == [] ->
          true

        matches_any_glob_pattern?(normalized_name, allow_patterns) ->
          true

        normalized_name == "apply_patch" and matches_any_glob_pattern?("exec", allow_patterns) ->
          true

        true ->
          false
      end
    end)
  end

  defp normalize_meta(nil), do: nil

  defp normalize_meta(meta) when is_map(meta) do
    plugin_id =
      meta
      |> Map.get(
        :plugin_id,
        Map.get(meta, "plugin_id", Map.get(meta, :pluginId, Map.get(meta, "pluginId")))
      )
      |> normalize_plugin_id()

    if plugin_id == "", do: nil, else: plugin_id
  end

  defp normalize_meta(_), do: nil

  defp normalize_plugin_id(value) when is_binary(value),
    do: value |> String.trim() |> String.downcase()

  defp normalize_plugin_id(_), do: ""

  defp normalize_policy(nil), do: nil

  defp normalize_policy(policy) when is_map(policy) do
    %{}
    |> maybe_put(:allow, Map.get(policy, :allow, Map.get(policy, "allow")))
    |> maybe_put(:deny, Map.get(policy, :deny, Map.get(policy, "deny")))
  end

  defp normalize_policy(_), do: nil

  defp policy_from(params, keys) do
    params
    |> first_present(keys)
    |> normalize_policy()
  end

  defp optional_trim(params, keys) do
    params
    |> first_present(keys)
    |> case do
      value when is_binary(value) ->
        value = String.trim(value)
        if value == "", do: nil, else: value

      _ ->
        nil
    end
  end

  defp default_tool_meta(_tool), do: nil

  defp first_present(_params, []), do: nil

  defp first_present(params, [key | rest]) do
    if Map.has_key?(params, key) do
      Map.get(params, key)
    else
      first_present(params, rest)
    end
  end

  defp tool_name(%{name: name}) when is_binary(name), do: name
  defp tool_name(%{"name" => name}) when is_binary(name), do: name
  defp tool_name(_), do: ""

  defp compile_glob_patterns(raw_patterns) when is_list(raw_patterns) do
    raw_patterns
    |> Enum.map(&compile_glob_pattern/1)
    |> Enum.reject(&(&1 == :empty_exact))
  end

  defp compile_glob_pattern(raw) when is_binary(raw) do
    normalized = OpenClawToolPolicy.normalize_tool_name(raw)

    cond do
      normalized == "" ->
        :empty_exact

      normalized == "*" ->
        :all

      String.contains?(normalized, "*") ->
        escaped =
          normalized
          |> Regex.escape()
          |> String.replace("\\*", ".*")

        {:regex, Regex.compile!("^#{escaped}$")}

      true ->
        {:exact, normalized}
    end
  end

  defp compile_glob_pattern(_), do: :empty_exact

  defp matches_any_glob_pattern?(_value, []), do: false

  defp matches_any_glob_pattern?(value, patterns) when is_binary(value) do
    Enum.any?(patterns, fn
      :all -> true
      {:exact, exact} -> value == exact
      {:regex, regex} -> Regex.match?(regex, value)
      _ -> false
    end)
  end

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)
end
