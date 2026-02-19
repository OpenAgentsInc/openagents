defmodule OpenAgentsRuntime.Parity.OpenClawToolPolicyParityTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Policy.OpenClawToolPolicy

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
    |> Map.new()
  end

  defp normalize_key(:by_plugin), do: "byPlugin"
  defp normalize_key(:unknown_allowlist), do: "unknownAllowlist"
  defp normalize_key(:stripped_allowlist), do: "strippedAllowlist"
  defp normalize_key(key) when is_atom(key), do: Atom.to_string(key)
  defp normalize_key(key), do: to_string(key)
end
