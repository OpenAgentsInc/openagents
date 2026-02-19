defmodule OpenAgentsRuntime.Parity.OpenClawToolPolicyParityTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Policy.OpenClawToolPolicy
  alias OpenAgentsRuntime.Tools.Policy.OpenClawToolPolicyPipeline

  @fixture_path Path.expand("../../fixtures/openclaw/tool_policy_parity_cases.json", __DIR__)

  test "runtime tool-policy baseline matches captured OpenClaw outputs" do
    fixture =
      @fixture_path
      |> File.read!()
      |> Jason.decode!()

    assert get_in(fixture, ["meta", "upstream", "repo"]) == "openclaw"
    assert is_binary(get_in(fixture, ["meta", "upstream", "commit"]))

    Enum.each(fixture["cases"] || [], fn test_case ->
      actual = eval_case(test_case)
      expected = test_case["expected_openclaw"]

      assert normalize(actual) == normalize(expected),
             "Parity mismatch for case #{test_case["id"]}"
    end)
  end

  defp eval_case(%{"operation" => "normalize_tool_name", "input" => %{"name" => name}}) do
    OpenClawToolPolicy.normalize_tool_name(name)
  end

  defp eval_case(%{"operation" => "expand_tool_groups", "input" => %{"list" => list}}) do
    OpenClawToolPolicy.expand_tool_groups(list)
  end

  defp eval_case(%{"operation" => "build_plugin_tool_groups", "input" => %{"tools" => tools}}) do
    OpenClawToolPolicy.build_plugin_tool_groups(tools)
  end

  defp eval_case(%{"operation" => "expand_policy_with_plugin_groups", "input" => input}) do
    OpenClawToolPolicy.expand_policy_with_plugin_groups(input["policy"], input["groups"])
  end

  defp eval_case(%{"operation" => "strip_plugin_only_allowlist", "input" => input}) do
    core_tools = input["core_tools"] |> List.wrap() |> MapSet.new()

    OpenClawToolPolicy.strip_plugin_only_allowlist(input["policy"], input["groups"], core_tools)
  end

  defp eval_case(%{"operation" => "build_default_tool_policy_pipeline_steps", "input" => input}) do
    OpenClawToolPolicyPipeline.build_default_tool_policy_pipeline_steps(input)
  end

  defp eval_case(%{"operation" => "apply_tool_policy_pipeline", "input" => input}) do
    warnings =
      input
      |> Map.get("warnings")
      |> case do
        pid when is_pid(pid) -> pid
        _ -> self()
      end

    tool_meta = fn tool ->
      case tool do
        %{"plugin_id" => plugin_id} when is_binary(plugin_id) -> %{plugin_id: plugin_id}
        %{"pluginId" => plugin_id} when is_binary(plugin_id) -> %{plugin_id: plugin_id}
        %{plugin_id: plugin_id} when is_binary(plugin_id) -> %{plugin_id: plugin_id}
        %{pluginId: plugin_id} when is_binary(plugin_id) -> %{plugin_id: plugin_id}
        _ -> nil
      end
    end

    filtered =
      OpenClawToolPolicyPipeline.apply_tool_policy_pipeline(%{
        tools: List.wrap(input["tools"]),
        steps: List.wrap(input["steps"]),
        tool_meta: tool_meta,
        warn: fn message -> send(warnings, {:policy_warning, message}) end
      })

    warning_messages =
      Stream.repeatedly(fn ->
        receive do
          {:policy_warning, message} -> {:ok, message}
        after
          0 -> :done
        end
      end)
      |> Enum.take_while(&(&1 != :done))
      |> Enum.map(fn {:ok, message} -> message end)

    %{
      "tools" =>
        Enum.map(filtered, fn tool ->
          tool["name"] || tool[:name]
        end),
      "warnings" => warning_messages
    }
  end

  defp eval_case(%{"operation" => unknown}) do
    flunk("Unknown parity operation: #{unknown}")
  end

  defp normalize(nil), do: nil
  defp normalize(value) when is_binary(value), do: value
  defp normalize(value) when is_boolean(value), do: value
  defp normalize(value) when is_number(value), do: value

  defp normalize(list) when is_list(list) do
    Enum.map(list, &normalize/1)
  end

  defp normalize(map) when is_map(map) do
    map
    |> Enum.map(fn {k, v} -> {normalize_key(k), normalize(v)} end)
    |> Enum.reject(fn {_k, v} -> is_nil(v) end)
    |> Map.new()
  end

  defp normalize_key(:by_plugin), do: "byPlugin"
  defp normalize_key(:unknown_allowlist), do: "unknownAllowlist"
  defp normalize_key(:stripped_allowlist), do: "strippedAllowlist"
  defp normalize_key(:strip_plugin_only_allowlist), do: "stripPluginOnlyAllowlist"
  defp normalize_key(key) when is_atom(key), do: Atom.to_string(key)
  defp normalize_key(key), do: to_string(key)
end
