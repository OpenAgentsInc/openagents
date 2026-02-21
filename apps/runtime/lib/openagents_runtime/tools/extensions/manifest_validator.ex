defmodule OpenAgentsRuntime.Tools.Extensions.ManifestValidator do
  @moduledoc """
  Generic extension manifest validator for runtime activation boundaries.

  Contract references:
  - docs/protocol/extensions/extension-manifest.schema.v1.json
  - docs/protocol/comms/integration-manifest.schema.v1.json (comms specialization)
  """

  @allowed_statuses MapSet.new(["active", "inactive", "error"])
  @id_pattern ~r/^[a-zA-Z0-9._:-]+$/

  @type validation_error :: %{
          required(String.t()) => term()
        }

  @type manifest_map :: %{optional(String.t()) => term()}

  @spec validate(map()) :: {:ok, manifest_map()} | {:error, [validation_error()]}
  def validate(manifest) when is_map(manifest) do
    manifest = stringify_keys(manifest)

    errors =
      []
      |> validate_manifest_version(manifest)
      |> validate_extension_identity(manifest)
      |> validate_tool_pack(manifest)
      |> validate_provider(manifest)
      |> validate_status(manifest)
      |> validate_capabilities(manifest)
      |> validate_metadata(manifest)
      |> Enum.reverse()

    if errors == [] do
      {:ok, normalize_manifest(manifest)}
    else
      {:error, errors}
    end
  end

  def validate(_), do: {:error, [invalid_schema("manifest", "manifest must be an object")]}

  defp validate_manifest_version(errors, manifest) do
    value = manifest["manifest_version"]

    if is_binary(value) and String.trim(value) != "" do
      errors
    else
      [
        invalid_schema("manifest_version", "manifest_version must be a non-empty string")
        | errors
      ]
    end
  end

  defp validate_extension_identity(errors, manifest) do
    extension_id = normalize_string(manifest["extension_id"])
    integration_id = normalize_string(manifest["integration_id"])
    canonical_id = extension_id || integration_id

    errors =
      cond do
        is_nil(canonical_id) ->
          [
            invalid_schema(
              "extension_id",
              "extension_id or integration_id is required"
            )
            | errors
          ]

        true ->
          errors
      end

    errors =
      if present_ids_match?(extension_id, integration_id) do
        errors
      else
        [
          invalid_schema(
            "extension_id",
            "extension_id must match integration_id when both are set"
          )
          | errors
        ]
      end

    validate_id_format(errors, canonical_id)
  end

  defp validate_tool_pack(errors, manifest) do
    value = manifest["tool_pack"]

    if is_binary(value) and String.trim(value) != "" do
      errors
    else
      [invalid_schema("tool_pack", "tool_pack must be a non-empty string") | errors]
    end
  end

  defp validate_provider(errors, manifest) do
    value = manifest["provider"]

    if is_binary(value) and String.trim(value) != "" do
      errors
    else
      [invalid_schema("provider", "provider must be a non-empty string") | errors]
    end
  end

  defp validate_status(errors, manifest) do
    case manifest["status"] do
      value when is_binary(value) ->
        if MapSet.member?(@allowed_statuses, value) do
          errors
        else
          [
            invalid_schema(
              "status",
              "status is not allowed: #{inspect(value)} (expected active|inactive|error)"
            )
            | errors
          ]
        end

      _ ->
        [invalid_schema("status", "status must be a string") | errors]
    end
  end

  defp validate_capabilities(errors, manifest) do
    case manifest["capabilities"] do
      values when is_list(values) ->
        errors
        |> validate_capability_entries(values)
        |> validate_capability_uniqueness(values)

      _ ->
        [invalid_schema("capabilities", "capabilities must be an array of strings") | errors]
    end
  end

  defp validate_capability_entries(errors, values) do
    if Enum.all?(values, &(is_binary(&1) and String.trim(&1) != "")) do
      errors
    else
      [invalid_schema("capabilities", "capabilities entries must be non-empty strings") | errors]
    end
  end

  defp validate_capability_uniqueness(errors, values) do
    if dedupe_preserving_order(values) == values do
      errors
    else
      [invalid_schema("capabilities", "capabilities must be unique") | errors]
    end
  end

  defp validate_metadata(errors, manifest) do
    case manifest["metadata"] do
      nil -> errors
      value when is_map(value) -> errors
      _ -> [invalid_schema("metadata", "metadata must be an object when present") | errors]
    end
  end

  defp validate_id_format(errors, nil), do: errors

  defp validate_id_format(errors, id) do
    cond do
      String.length(id) < 3 or String.length(id) > 128 ->
        [invalid_schema("extension_id", "extension_id length must be between 3 and 128") | errors]

      not String.match?(id, @id_pattern) ->
        [invalid_schema("extension_id", "extension_id contains invalid characters") | errors]

      true ->
        errors
    end
  end

  defp normalize_manifest(manifest) do
    extension_id = normalize_string(manifest["extension_id"] || manifest["integration_id"])

    manifest
    |> Map.put("extension_id", extension_id)
    |> Map.put("integration_id", normalize_string(manifest["integration_id"]) || extension_id)
    |> Map.update("capabilities", [], &dedupe_preserving_order(List.wrap(&1)))
    |> Map.put("metadata", stringify_keys(manifest["metadata"]) || %{})
  end

  defp normalize_string(value) when is_binary(value) do
    trimmed = String.trim(value)
    if trimmed == "", do: nil, else: trimmed
  end

  defp normalize_string(nil), do: nil

  defp normalize_string(value) when is_atom(value) and value not in [true, false],
    do: value |> Atom.to_string() |> normalize_string()

  defp normalize_string(_value), do: nil

  defp present_ids_match?(nil, _integration_id), do: true
  defp present_ids_match?(_extension_id, nil), do: true
  defp present_ids_match?(extension_id, integration_id), do: extension_id == integration_id

  defp invalid_schema(path, message) do
    %{
      "reason_code" => "manifest_validation.invalid_schema",
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

  defp dedupe_preserving_order(values) when is_list(values) do
    {deduped, _seen} =
      Enum.reduce(values, {[], MapSet.new()}, fn value, {acc, seen} ->
        if MapSet.member?(seen, value) do
          {acc, seen}
        else
          {[value | acc], MapSet.put(seen, value)}
        end
      end)

    Enum.reverse(deduped)
  end
end
