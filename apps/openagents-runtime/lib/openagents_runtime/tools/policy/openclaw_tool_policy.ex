defmodule OpenAgentsRuntime.Tools.Policy.OpenClawToolPolicy do
  @moduledoc """
  Baseline Elixir port of OpenClaw tool-policy normalization helpers.

  Upstream reference:
  - /Users/christopherdavid/code/openclaw/src/agents/tool-policy.ts
  - commit 8e1f25631b220f139e79003caecabd11b7e1e748
  """

  @tool_name_aliases %{
    "apply-patch" => "apply_patch",
    "bash" => "exec"
  }

  @tool_groups %{
    "group:memory" => ["memory_search", "memory_get"],
    "group:web" => ["web_search", "web_fetch"],
    "group:fs" => ["read", "write", "edit", "apply_patch"],
    "group:runtime" => ["exec", "process"],
    "group:sessions" => [
      "sessions_list",
      "sessions_history",
      "sessions_send",
      "sessions_spawn",
      "subagents",
      "session_status"
    ],
    "group:ui" => ["browser", "canvas"],
    "group:automation" => ["cron", "gateway"],
    "group:messaging" => ["message"],
    "group:nodes" => ["nodes"],
    "group:openclaw" => [
      "browser",
      "canvas",
      "nodes",
      "cron",
      "message",
      "gateway",
      "agents_list",
      "sessions_list",
      "sessions_history",
      "sessions_send",
      "sessions_spawn",
      "subagents",
      "session_status",
      "memory_search",
      "memory_get",
      "web_search",
      "web_fetch",
      "image"
    ]
  }

  @type tool_policy :: %{
          optional(:allow) => [String.t()],
          optional(:deny) => [String.t()]
        }

  @type plugin_group_input :: %{
          optional(:all) => [String.t()],
          optional(:by_plugin) => %{optional(String.t()) => [String.t()]}
        }

  @type plugin_groups :: %{
          all: [String.t()],
          by_plugin: %{optional(String.t()) => [String.t()]}
        }

  @type allowlist_resolution :: %{
          policy: nil | tool_policy(),
          unknown_allowlist: [String.t()],
          stripped_allowlist: boolean()
        }

  @spec normalize_tool_name(String.t()) :: String.t()
  def normalize_tool_name(name) when is_binary(name) do
    normalized = name |> String.trim() |> String.downcase()
    Map.get(@tool_name_aliases, normalized, normalized)
  end

  @spec normalize_tool_list([String.t()] | nil) :: [String.t()]
  def normalize_tool_list(list) when is_list(list) do
    list
    |> Enum.filter(&is_binary/1)
    |> Enum.map(&normalize_tool_name/1)
    |> Enum.reject(&(&1 == ""))
  end

  def normalize_tool_list(_), do: []

  @spec expand_tool_groups([String.t()] | nil) :: [String.t()]
  def expand_tool_groups(list) do
    list
    |> normalize_tool_list()
    |> Enum.flat_map(fn value -> Map.get(@tool_groups, value, [value]) end)
    |> dedupe_preserving_order()
  end

  @spec build_plugin_tool_groups([map()]) :: plugin_groups()
  def build_plugin_tool_groups(tools) when is_list(tools) do
    tools
    |> Enum.reduce(%{all: [], by_plugin: %{}}, fn tool, acc ->
      with {:ok, name} <- tool_name(tool),
           {:ok, plugin_id} <- tool_plugin_id(tool) do
        normalized_name = normalize_tool_name(name)
        normalized_plugin_id = plugin_id |> String.trim() |> String.downcase()

        %{
          all: [normalized_name | acc.all],
          by_plugin:
            Map.update(
              acc.by_plugin,
              normalized_plugin_id,
              [normalized_name],
              &[normalized_name | &1]
            )
        }
      else
        _ -> acc
      end
    end)
    |> finalize_plugin_groups()
  end

  @spec expand_plugin_groups([String.t()] | nil, plugin_group_input()) :: [String.t()] | nil
  def expand_plugin_groups(nil, _groups), do: nil
  def expand_plugin_groups([], _groups), do: []

  def expand_plugin_groups(list, groups) when is_list(list) do
    groups = normalize_plugin_groups(groups)

    list
    |> normalize_tool_list()
    |> Enum.flat_map(fn entry ->
      cond do
        entry == "group:plugins" ->
          if groups.all == [], do: [entry], else: groups.all

        Map.has_key?(groups.by_plugin, entry) and groups.by_plugin[entry] != [] ->
          groups.by_plugin[entry]

        true ->
          [entry]
      end
    end)
    |> dedupe_preserving_order()
  end

  @spec expand_policy_with_plugin_groups(tool_policy() | nil, plugin_group_input()) ::
          tool_policy() | nil
  def expand_policy_with_plugin_groups(nil, _groups), do: nil

  def expand_policy_with_plugin_groups(policy, groups) when is_map(policy) do
    policy = normalize_policy(policy)
    groups = normalize_plugin_groups(groups)

    %{}
    |> maybe_put(:allow, expand_plugin_groups(policy[:allow], groups))
    |> maybe_put(:deny, expand_plugin_groups(policy[:deny], groups))
  end

  @spec strip_plugin_only_allowlist(
          tool_policy() | nil,
          plugin_group_input(),
          MapSet.t(String.t())
        ) ::
          allowlist_resolution()
  def strip_plugin_only_allowlist(policy, groups, core_tools)
      when (is_map(policy) or is_nil(policy)) and is_struct(core_tools, MapSet) do
    policy = normalize_policy(policy)
    groups = normalize_plugin_groups(groups)
    allowlist = policy[:allow]

    if is_nil(allowlist) or allowlist == [] do
      %{policy: policy, unknown_allowlist: [], stripped_allowlist: false}
    else
      normalized_allowlist = normalize_tool_list(allowlist)

      if normalized_allowlist == [] do
        %{policy: policy, unknown_allowlist: [], stripped_allowlist: false}
      else
        plugin_ids = groups.by_plugin |> Map.keys() |> MapSet.new()
        plugin_tools = MapSet.new(groups.all)

        {unknown_allowlist, has_core_entry} =
          Enum.reduce(normalized_allowlist, {[], false}, fn entry, {unknown, has_core?} ->
            cond do
              entry == "*" ->
                {unknown, true}

              true ->
                is_plugin_entry =
                  entry == "group:plugins" or
                    MapSet.member?(plugin_ids, entry) or
                    MapSet.member?(plugin_tools, entry)

                expanded = expand_tool_groups([entry])
                is_core_entry = Enum.any?(expanded, &MapSet.member?(core_tools, &1))
                next_has_core = has_core? or is_core_entry

                next_unknown =
                  if not is_core_entry and not is_plugin_entry do
                    [entry | unknown]
                  else
                    unknown
                  end

                {next_unknown, next_has_core}
            end
          end)

        stripped_allowlist = not has_core_entry

        updated_policy =
          if stripped_allowlist do
            Map.delete(policy, :allow)
          else
            policy
          end

        %{
          policy: updated_policy,
          unknown_allowlist: unknown_allowlist |> Enum.reverse() |> dedupe_preserving_order(),
          stripped_allowlist: stripped_allowlist
        }
      end
    end
  end

  defp tool_name(%{name: name}) when is_binary(name), do: {:ok, name}
  defp tool_name(%{"name" => name}) when is_binary(name), do: {:ok, name}
  defp tool_name(_), do: :error

  defp tool_plugin_id(%{plugin_id: plugin_id}) when is_binary(plugin_id), do: {:ok, plugin_id}
  defp tool_plugin_id(%{"plugin_id" => plugin_id}) when is_binary(plugin_id), do: {:ok, plugin_id}

  defp tool_plugin_id(%{pluginId: plugin_id}) when is_binary(plugin_id), do: {:ok, plugin_id}

  defp tool_plugin_id(%{"pluginId" => plugin_id}) when is_binary(plugin_id),
    do: {:ok, plugin_id}

  defp tool_plugin_id(_), do: :error

  defp finalize_plugin_groups(%{all: all, by_plugin: by_plugin}) do
    normalized_by_plugin =
      Map.new(by_plugin, fn {plugin_id, names} ->
        {plugin_id, names |> Enum.reverse() |> dedupe_preserving_order()}
      end)

    %{
      all: all |> Enum.reverse() |> dedupe_preserving_order(),
      by_plugin: normalized_by_plugin
    }
  end

  defp normalize_plugin_groups(groups) when is_map(groups) do
    all =
      groups
      |> Map.get(:all, Map.get(groups, "all", []))
      |> normalize_tool_list()

    raw_by_plugin =
      Map.get(groups, :by_plugin, Map.get(groups, "by_plugin", Map.get(groups, "byPlugin", %{})))

    by_plugin =
      if is_map(raw_by_plugin) do
        Map.new(raw_by_plugin, fn {plugin_id, names} ->
          {plugin_id |> to_string() |> String.trim() |> String.downcase(),
           normalize_tool_list(names)}
        end)
      else
        %{}
      end

    %{
      all: dedupe_preserving_order(all),
      by_plugin: by_plugin
    }
  end

  defp normalize_plugin_groups(_), do: %{all: [], by_plugin: %{}}

  defp normalize_policy(nil), do: nil

  defp normalize_policy(policy) when is_map(policy) do
    %{}
    |> maybe_put(:allow, Map.get(policy, :allow, Map.get(policy, "allow")))
    |> maybe_put(:deny, Map.get(policy, :deny, Map.get(policy, "deny")))
  end

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)

  defp dedupe_preserving_order(list) do
    {items, _set} =
      Enum.reduce(list, {[], MapSet.new()}, fn item, {acc, seen} ->
        if MapSet.member?(seen, item) do
          {acc, seen}
        else
          {[item | acc], MapSet.put(seen, item)}
        end
      end)

    Enum.reverse(items)
  end
end
