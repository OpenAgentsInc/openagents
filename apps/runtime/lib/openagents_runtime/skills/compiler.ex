defmodule OpenAgentsRuntime.Skills.Compiler do
  @moduledoc """
  Deterministic compiler from JSON SkillSpec -> Agent Skills-compatible bundle artifact.
  """

  @spec compile(map(), [map()]) :: map()
  def compile(skill_spec, tool_specs \\ []) when is_map(skill_spec) and is_list(tool_specs) do
    skill_spec = stringify_keys(skill_spec)
    root = slugify(skill_spec["name"] || skill_spec["skill_id"] || "skill")

    skill_md = build_skill_md(skill_spec, tool_specs)

    directories =
      %{}
      |> maybe_put_dir("#{root}/scripts", skill_spec["scripts"])
      |> maybe_put_dir("#{root}/references", skill_spec["references"])
      |> maybe_put_dir("#{root}/assets", skill_spec["assets"])

    %{
      "bundle_format" => "agent_skills.v1",
      "root" => root,
      "files" => %{"#{root}/SKILL.md" => skill_md},
      "directories" => directories,
      "tool_manifest_refs" =>
        Enum.map(tool_specs, fn tool_spec ->
          manifest = stringify_keys(tool_spec["integration_manifest"]) || %{}

          %{
            "tool_id" => tool_spec["tool_id"],
            "version" => tool_spec["version"],
            "integration_id" => manifest["integration_id"],
            "tool_pack" => tool_spec["tool_pack"]
          }
        end)
    }
  end

  defp build_skill_md(skill_spec, tool_specs) do
    frontmatter =
      %{}
      |> Map.put("name", skill_spec["name"])
      |> Map.put("description", skill_spec["description"])
      |> maybe_put("license", skill_spec["license"])
      |> maybe_put("compatibility", stringify_keys(skill_spec["compatibility"]) || %{})
      |> maybe_put("metadata", stringify_keys(skill_spec["metadata"]) || %{})
      |> maybe_put(
        "allowed-tools",
        Enum.map(tool_specs, fn tool_spec ->
          "#{tool_spec["tool_id"]}@v#{tool_spec["version"]}"
        end)
      )

    instructions = skill_spec["instructions_markdown"] || ""

    [
      "---",
      to_yaml(frontmatter),
      "---",
      "",
      instructions,
      ""
    ]
    |> Enum.join("\n")
  end

  defp to_yaml(map) when is_map(map) do
    map
    |> Enum.flat_map(fn {key, value} -> yaml_lines(key, value, 0) end)
    |> Enum.join("\n")
  end

  defp yaml_lines(key, value, indent) when is_binary(value) do
    [indent_spaces(indent) <> "#{key}: #{escape_string(value)}"]
  end

  defp yaml_lines(key, value, indent) when is_integer(value) do
    [indent_spaces(indent) <> "#{key}: #{value}"]
  end

  defp yaml_lines(key, value, indent) when is_boolean(value) do
    [indent_spaces(indent) <> "#{key}: #{value}"]
  end

  defp yaml_lines(key, value, indent) when is_list(value) do
    header = indent_spaces(indent) <> "#{key}:"

    items =
      Enum.flat_map(value, fn
        entry when is_map(entry) ->
          ([indent_spaces(indent + 2) <> "-"] ++
             entry)
          |> Enum.flat_map(fn {entry_key, entry_value} ->
            yaml_lines(entry_key, entry_value, indent + 4)
          end)

        entry ->
          [indent_spaces(indent + 2) <> "- #{escape_string(to_string(entry))}"]
      end)

    [header | items]
  end

  defp yaml_lines(key, value, indent) when is_map(value) do
    header = indent_spaces(indent) <> "#{key}:"

    children =
      Enum.flat_map(value, fn {child_key, child_value} ->
        yaml_lines(child_key, child_value, indent + 2)
      end)

    [header | children]
  end

  defp yaml_lines(key, _value, indent) do
    [indent_spaces(indent) <> "#{key}: null"]
  end

  defp maybe_put(map, _key, nil), do: map

  defp maybe_put(map, key, value) when is_map(value) do
    if map_size(value) == 0, do: map, else: Map.put(map, key, value)
  end

  defp maybe_put(map, key, value) when is_list(value) do
    if value == [], do: map, else: Map.put(map, key, value)
  end

  defp maybe_put(map, key, value), do: Map.put(map, key, value)

  defp maybe_put_dir(map, _path, values) when values in [nil, []], do: map

  defp maybe_put_dir(map, path, values) when is_list(values) do
    Map.put(map, path, Enum.map(values, &(stringify_keys(&1) || %{})))
  end

  defp maybe_put_dir(map, path, value) when is_map(value) do
    Map.put(map, path, [stringify_keys(value)])
  end

  defp maybe_put_dir(map, _path, _value), do: map

  defp slugify(value) do
    value
    |> to_string()
    |> String.downcase()
    |> String.replace(~r/[^a-z0-9]+/, "-")
    |> String.trim("-")
    |> case do
      "" -> "skill"
      slug -> slug
    end
  end

  defp escape_string(value) do
    escaped =
      value
      |> String.replace("\\", "\\\\")
      |> String.replace("\"", "\\\"")

    if String.contains?(escaped, [":", "#", "\n"]) do
      "\"#{escaped}\""
    else
      escaped
    end
  end

  defp indent_spaces(count), do: String.duplicate(" ", count)

  defp stringify_keys(nil), do: nil

  defp stringify_keys(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end
end
