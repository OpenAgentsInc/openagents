defmodule OpenAgentsRuntime.Skills.Compatibility do
  @moduledoc """
  Agent Skills compatibility checks for compiled bundle artifacts.

  The checks validate canonical bundle shape expected by `skills-ref validate`:
  `skill-root/SKILL.md` plus optional `scripts/`, `references/`, and `assets/` directories.
  """

  @type error_map :: %{required(String.t()) => term()}

  @spec validate_bundle(map()) :: {:ok, map()} | {:error, [error_map()]}
  def validate_bundle(bundle) when is_map(bundle) do
    bundle = stringify_keys(bundle)
    root = normalize_string(bundle["root"])
    files = stringify_keys(bundle["files"])
    directories = stringify_keys(bundle["directories"])

    errors =
      []
      |> maybe_add_error(is_nil(root), "root", "root is required")
      |> maybe_add_error(not is_map(files), "files", "files must be an object")
      |> maybe_add_error(not is_map(directories), "directories", "directories must be an object")
      |> validate_skill_md(root, files)
      |> validate_optional_dirs(root, directories)
      |> Enum.reverse()

    if errors == [] do
      {:ok,
       %{
         "validator" => "agent_skills_shape.v1",
         "compatible" => true,
         "root" => root,
         "required_files" => ["#{root}/SKILL.md"],
         "checked_at" => DateTime.utc_now() |> DateTime.truncate(:second) |> DateTime.to_iso8601()
       }}
    else
      {:error, errors}
    end
  end

  def validate_bundle(_), do: {:error, [invalid("bundle", "bundle must be an object")]}

  defp validate_skill_md(errors, nil, _files), do: errors
  defp validate_skill_md(errors, _root, files) when not is_map(files), do: errors

  defp validate_skill_md(errors, root, files) do
    skill_md_path = "#{root}/SKILL.md"

    case files[skill_md_path] do
      value when is_binary(value) ->
        if String.trim(value) == "" do
          [invalid("files.#{skill_md_path}", "SKILL.md must not be empty") | errors]
        else
          validate_frontmatter(errors, value)
        end

      _ ->
        [invalid("files.#{skill_md_path}", "SKILL.md is required") | errors]
    end
  end

  defp validate_frontmatter(errors, skill_md) do
    with [_, frontmatter_block] <- Regex.run(~r/\A---\n(.*?)\n---\n/s, skill_md),
         frontmatter when is_binary(frontmatter) <- frontmatter_block do
      errors
      |> maybe_add_error(
        not has_frontmatter_field?(frontmatter, "name"),
        "frontmatter.name",
        "name is required"
      )
      |> maybe_add_error(
        not has_frontmatter_field?(frontmatter, "description"),
        "frontmatter.description",
        "description is required"
      )
    else
      _ -> [invalid("SKILL.md", "missing frontmatter block") | errors]
    end
  end

  defp validate_optional_dirs(errors, nil, _directories), do: errors
  defp validate_optional_dirs(errors, _root, directories) when not is_map(directories), do: errors

  defp validate_optional_dirs(errors, root, directories) do
    Enum.reduce(~w(scripts references assets), errors, fn dirname, acc ->
      path = "#{root}/#{dirname}"

      case Map.fetch(directories, path) do
        :error ->
          acc

        {:ok, entries} when is_list(entries) ->
          if Enum.all?(entries, &is_map/1) do
            acc
          else
            [invalid("directories.#{path}", "must contain objects") | acc]
          end

        {:ok, _other} ->
          [invalid("directories.#{path}", "must be an array") | acc]
      end
    end)
  end

  defp has_frontmatter_field?(frontmatter, field) do
    Regex.match?(~r/^#{Regex.escape(field)}:\s+.+$/m, frontmatter)
  end

  defp maybe_add_error(errors, false, _path, _message), do: errors
  defp maybe_add_error(errors, true, path, message), do: [invalid(path, message) | errors]

  defp invalid(path, message) do
    %{
      "reason_code" => "skill_registry.compatibility_failed",
      "path" => path,
      "message" => message
    }
  end

  defp normalize_string(value) when is_binary(value) do
    value
    |> String.trim()
    |> case do
      "" -> nil
      trimmed -> trimmed
    end
  end

  defp normalize_string(_), do: nil

  defp stringify_keys(nil), do: %{}

  defp stringify_keys(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end
end
