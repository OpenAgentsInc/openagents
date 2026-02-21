defmodule OpenAgentsRuntime.Skills.SpecValidator do
  @moduledoc """
  Deterministic validation for JSON ToolSpec/SkillSpec payloads.
  """

  alias OpenAgentsRuntime.Tools.Extensions.ManifestRegistry

  @tool_id_pattern ~r/^[a-zA-Z0-9._:-]+$/
  @pricing_models MapSet.new(~w(free fixed_per_call metered subscription))
  @currencies MapSet.new(["BTC_SATS"])
  @settlement_modes MapSet.new(~w(prepaid postpaid_authorized))

  @type error_map :: %{
          required(String.t()) => term()
        }

  @spec validate_tool_spec(map()) :: {:ok, map()} | {:error, [error_map()]}
  def validate_tool_spec(tool_spec) when is_map(tool_spec) do
    tool_spec = stringify_keys(tool_spec)

    errors =
      []
      |> validate_required_string(tool_spec, "tool_id")
      |> validate_required_integer(tool_spec, "version", min: 1)
      |> validate_required_string(tool_spec, "name")
      |> validate_required_string(tool_spec, "description")
      |> validate_required_string(tool_spec, "tool_pack")
      |> validate_required_string(tool_spec, "execution_kind")
      |> validate_identifier(tool_spec, "tool_id")
      |> validate_map_field(tool_spec, "input_schema")
      |> validate_map_field(tool_spec, "output_schema")
      |> validate_map_field(tool_spec, "auth_requirements")
      |> validate_map_field(tool_spec, "safety_policy")
      |> validate_commercial(tool_spec, "commercial")
      |> validate_integration_manifest(tool_spec)
      |> Enum.reverse()

    if errors == [] do
      {:ok, normalize_tool_spec(tool_spec)}
    else
      {:error, errors}
    end
  end

  def validate_tool_spec(_), do: {:error, [invalid_schema("tool_spec", "must be an object")]}

  @spec validate_skill_spec(map()) :: {:ok, map()} | {:error, [error_map()]}
  def validate_skill_spec(skill_spec) when is_map(skill_spec) do
    skill_spec = stringify_keys(skill_spec)

    errors =
      []
      |> validate_required_string(skill_spec, "skill_id")
      |> validate_required_integer(skill_spec, "version", min: 1)
      |> validate_required_string(skill_spec, "name")
      |> validate_required_string(skill_spec, "description")
      |> validate_required_string(skill_spec, "instructions_markdown")
      |> validate_identifier(skill_spec, "skill_id")
      |> validate_optional_string(skill_spec, "license")
      |> validate_map_field(skill_spec, "compatibility")
      |> validate_map_field(skill_spec, "metadata")
      |> validate_commercial(skill_spec, "commercial")
      |> validate_tool_refs(skill_spec)
      |> validate_descriptor_array(skill_spec, "scripts")
      |> validate_descriptor_array(skill_spec, "references")
      |> validate_descriptor_array(skill_spec, "assets")
      |> Enum.reverse()

    if errors == [] do
      {:ok, normalize_skill_spec(skill_spec)}
    else
      {:error, errors}
    end
  end

  def validate_skill_spec(_), do: {:error, [invalid_schema("skill_spec", "must be an object")]}

  defp validate_required_string(errors, map, key) do
    case normalize_string(map[key]) do
      nil -> [invalid_schema(key, "is required") | errors]
      _value -> errors
    end
  end

  defp validate_optional_string(errors, map, key) do
    value = Map.get(map, key)

    cond do
      is_nil(value) ->
        errors

      is_binary(value) and String.trim(value) != "" ->
        errors

      true ->
        [invalid_schema(key, "must be a non-empty string when provided") | errors]
    end
  end

  defp validate_required_integer(errors, map, key, opts) do
    min = Keyword.get(opts, :min, 0)

    case map[key] do
      value when is_integer(value) and value >= min ->
        errors

      _ ->
        [invalid_schema(key, "must be an integer >= #{min}") | errors]
    end
  end

  defp validate_identifier(errors, map, key) do
    case normalize_string(map[key]) do
      nil ->
        errors

      value ->
        cond do
          String.length(value) < 3 or String.length(value) > 128 ->
            [invalid_schema(key, "length must be between 3 and 128") | errors]

          not String.match?(value, @tool_id_pattern) ->
            [invalid_schema(key, "contains invalid characters") | errors]

          true ->
            errors
        end
    end
  end

  defp validate_map_field(errors, map, key) do
    case Map.get(map, key) do
      nil -> errors
      value when is_map(value) -> errors
      _ -> [invalid_schema(key, "must be an object when provided") | errors]
    end
  end

  defp validate_descriptor_array(errors, map, key) do
    case Map.get(map, key, []) do
      nil ->
        errors

      value when is_list(value) ->
        if Enum.all?(value, &is_map/1) do
          errors
        else
          [invalid_schema(key, "must contain objects") | errors]
        end

      _ ->
        [invalid_schema(key, "must be an array of objects") | errors]
    end
  end

  defp validate_tool_refs(errors, skill_spec) do
    case Map.get(skill_spec, "allowed_tools", []) do
      refs when is_list(refs) ->
        Enum.reduce(Enum.with_index(refs), errors, fn {entry, index}, acc ->
          case stringify_keys(entry) do
            %{"tool_id" => tool_id} = map when is_binary(tool_id) ->
              if String.trim(tool_id) == "" do
                [invalid_schema("allowed_tools.#{index}.tool_id", "is required") | acc]
              else
                acc
                |> validate_required_integer(map, "version", min: 1)
                |> prefix_last_error("allowed_tools.#{index}.version")
              end

            _ ->
              [invalid_schema("allowed_tools.#{index}", "must include tool_id and version") | acc]
          end
        end)

      _ ->
        [invalid_schema("allowed_tools", "must be an array") | errors]
    end
  end

  defp prefix_last_error(errors, _path) when errors == [], do: errors

  defp prefix_last_error([head | tail], path) do
    [Map.put(head, "path", path) | tail]
  end

  defp validate_commercial(errors, map, key) do
    case stringify_keys(Map.get(map, key, %{})) do
      nil ->
        errors

      commercial when is_map(commercial) ->
        errors
        |> validate_pricing_model(commercial, key)
        |> validate_currency(commercial, key)
        |> validate_settlement_mode(commercial, key)
        |> validate_cap_integer(commercial, key, "max_per_call_sats")
        |> validate_cap_integer(commercial, key, "max_daily_sats")
        |> validate_cap_integer(commercial, key, "max_monthly_sats")

      _ ->
        [invalid_schema(key, "must be an object") | errors]
    end
  end

  defp validate_pricing_model(errors, commercial, key) do
    case normalize_string(commercial["pricing_model"]) do
      nil ->
        errors

      model ->
        if MapSet.member?(@pricing_models, model) do
          errors
        else
          [invalid_schema("#{key}.pricing_model", "is not supported") | errors]
        end
    end
  end

  defp validate_currency(errors, commercial, key) do
    case normalize_string(commercial["currency"]) do
      nil ->
        errors

      currency ->
        if MapSet.member?(@currencies, currency) do
          errors
        else
          [invalid_schema("#{key}.currency", "must be BTC_SATS") | errors]
        end
    end
  end

  defp validate_settlement_mode(errors, commercial, key) do
    case normalize_string(commercial["settlement_mode"]) do
      nil ->
        errors

      mode ->
        if MapSet.member?(@settlement_modes, mode) do
          errors
        else
          [invalid_schema("#{key}.settlement_mode", "is not supported") | errors]
        end
    end
  end

  defp validate_cap_integer(errors, commercial, key, field) do
    case commercial[field] do
      nil ->
        errors

      value when is_integer(value) and value >= 0 ->
        errors

      _ ->
        [invalid_schema("#{key}.#{field}", "must be an integer >= 0") | errors]
    end
  end

  defp validate_integration_manifest(errors, tool_spec) do
    manifest = stringify_keys(tool_spec["integration_manifest"])

    cond do
      not is_map(manifest) ->
        [invalid_schema("integration_manifest", "must be an object") | errors]

      true ->
        case ManifestRegistry.validate_for_activation(manifest) do
          {:ok, normalized_manifest} ->
            maybe_put_integration_manifest(errors, normalized_manifest)

          {:error, {:invalid_manifest, manifest_errors}} ->
            mapped =
              Enum.map(manifest_errors, fn err ->
                path = err["path"] || "manifest"
                message = err["message"] || "invalid manifest"
                invalid_schema("integration_manifest.#{path}", message)
              end)

            mapped ++ errors
        end
    end
  end

  defp maybe_put_integration_manifest(errors, _normalized_manifest), do: errors

  defp normalize_tool_spec(tool_spec) do
    tool_spec
    |> Map.put("tool_id", normalize_string(tool_spec["tool_id"]))
    |> Map.put("version", tool_spec["version"])
    |> Map.put("tool_pack", normalize_string(tool_spec["tool_pack"]))
    |> Map.put("name", normalize_string(tool_spec["name"]))
    |> Map.put("description", normalize_string(tool_spec["description"]))
    |> Map.put("execution_kind", normalize_string(tool_spec["execution_kind"]) || "http")
    |> Map.put("integration_manifest", stringify_keys(tool_spec["integration_manifest"]) || %{})
    |> Map.put("input_schema", stringify_keys(tool_spec["input_schema"]) || %{})
    |> Map.put("output_schema", stringify_keys(tool_spec["output_schema"]) || %{})
    |> Map.put("auth_requirements", stringify_keys(tool_spec["auth_requirements"]) || %{})
    |> Map.put("safety_policy", stringify_keys(tool_spec["safety_policy"]) || %{})
    |> Map.put("commercial", stringify_keys(tool_spec["commercial"]) || %{})
    |> Map.put("metadata", stringify_keys(tool_spec["metadata"]) || %{})
    |> Map.put("submitted_by", normalize_string(tool_spec["submitted_by"]))
  end

  defp normalize_skill_spec(skill_spec) do
    skill_spec
    |> Map.put("skill_id", normalize_string(skill_spec["skill_id"]))
    |> Map.put("version", skill_spec["version"])
    |> Map.put("name", normalize_string(skill_spec["name"]))
    |> Map.put("description", normalize_string(skill_spec["description"]))
    |> Map.put("license", normalize_string(skill_spec["license"]))
    |> Map.put("instructions_markdown", normalize_string(skill_spec["instructions_markdown"]))
    |> Map.put("compatibility", stringify_keys(skill_spec["compatibility"]) || %{})
    |> Map.put("allowed_tools", normalize_tool_refs(skill_spec["allowed_tools"]))
    |> Map.put("scripts", normalize_descriptor_array(skill_spec["scripts"]))
    |> Map.put("references", normalize_descriptor_array(skill_spec["references"]))
    |> Map.put("assets", normalize_descriptor_array(skill_spec["assets"]))
    |> Map.put("commercial", stringify_keys(skill_spec["commercial"]) || %{})
    |> Map.put("metadata", stringify_keys(skill_spec["metadata"]) || %{})
    |> Map.put("submitted_by", normalize_string(skill_spec["submitted_by"]))
  end

  defp normalize_tool_refs(refs) when is_list(refs) do
    Enum.map(refs, fn ref ->
      ref = stringify_keys(ref)

      %{
        "tool_id" => normalize_string(ref["tool_id"]),
        "version" => ref["version"]
      }
    end)
  end

  defp normalize_tool_refs(_), do: []

  defp normalize_descriptor_array(values) when is_list(values) do
    Enum.map(values, fn value -> stringify_keys(value) || %{} end)
  end

  defp normalize_descriptor_array(_), do: []

  defp normalize_string(value) when is_binary(value) do
    value
    |> String.trim()
    |> case do
      "" -> nil
      trimmed -> trimmed
    end
  end

  defp normalize_string(_), do: nil

  defp invalid_schema(path, message) do
    %{
      "reason_code" => "skill_registry.invalid_schema",
      "path" => path,
      "message" => message
    }
  end

  defp stringify_keys(nil), do: nil

  defp stringify_keys(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end
end
