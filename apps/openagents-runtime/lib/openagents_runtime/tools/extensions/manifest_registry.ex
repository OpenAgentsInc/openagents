defmodule OpenAgentsRuntime.Tools.Extensions.ManifestRegistry do
  @moduledoc """
  Runtime extension manifest registry/activation validator.

  Responsibilities:
  - run strict base manifest validation
  - route to tool-pack-specific validators
  - emit deterministic validation outcome telemetry
  - return machine-readable validation errors for control-plane/runtime callers
  """

  alias OpenAgentsRuntime.Telemetry.Events
  alias OpenAgentsRuntime.Tools.Extensions.CommsManifestValidator
  alias OpenAgentsRuntime.Tools.Extensions.ManifestValidator

  @base_contract_ref "docs/protocol/extensions/extension-manifest.schema.v1.json"

  @tool_pack_contract_refs %{
    "comms.v1" => [
      "docs/protocol/comms/integration-manifest.schema.v1.json",
      "docs/protocol/comms/tool-pack-contract.v1.json"
    ]
  }

  @validators %{
    "comms.v1" => CommsManifestValidator
  }

  @type validation_error :: %{
          required(String.t()) => term()
        }

  @spec validate_for_activation(map(), keyword()) ::
          {:ok, map()} | {:error, {:invalid_manifest, [validation_error()]}}
  def validate_for_activation(manifest, _opts \\ []) do
    with {:ok, base_manifest} <- ManifestValidator.validate(manifest),
         {:ok, validator} <- validator_for_tool_pack(base_manifest["tool_pack"]),
         {:ok, validated_manifest} <- validate_with_module(validator, base_manifest) do
      manifest_with_refs =
        validated_manifest
        |> normalize_success_manifest()
        |> put_contract_refs(base_manifest["tool_pack"])

      emit_validation_outcome("accepted", manifest_with_refs, [])
      {:ok, manifest_with_refs}
    else
      {:error, {:unsupported_tool_pack, tool_pack}} ->
        errors = [invalid_schema("tool_pack", "unsupported tool_pack: #{inspect(tool_pack)}")]
        emit_validation_outcome("rejected", stringify_keys(manifest), errors)
        {:error, {:invalid_manifest, errors}}

      {:error, errors} when is_list(errors) ->
        normalized_errors = normalize_errors(errors)
        emit_validation_outcome("rejected", stringify_keys(manifest), normalized_errors)
        {:error, {:invalid_manifest, normalized_errors}}
    end
  end

  @spec supported_tool_packs() :: [String.t()]
  def supported_tool_packs do
    @validators
    |> Map.keys()
    |> Enum.sort()
  end

  defp validator_for_tool_pack(tool_pack) when is_binary(tool_pack) do
    case Map.fetch(@validators, tool_pack) do
      {:ok, validator} -> {:ok, validator}
      :error -> {:error, {:unsupported_tool_pack, tool_pack}}
    end
  end

  defp validator_for_tool_pack(_tool_pack), do: {:error, {:unsupported_tool_pack, nil}}

  defp validate_with_module(module, manifest) when is_atom(module) do
    case module.validate(manifest) do
      {:ok, normalized} when is_map(normalized) ->
        {:ok, normalized}

      {:error, errors} when is_list(errors) ->
        {:error, errors}

      {:error, reason} ->
        {:error, [invalid_schema("manifest", "validator rejected manifest: #{inspect(reason)}")]}

      other ->
        {:error,
         [invalid_schema("manifest", "validator returned invalid response: #{inspect(other)}")]}
    end
  end

  defp normalize_success_manifest(manifest) when is_map(manifest) do
    manifest = stringify_keys(manifest)
    extension_id = manifest["extension_id"] || manifest["integration_id"]

    manifest
    |> Map.put("extension_id", extension_id)
    |> Map.put("integration_id", manifest["integration_id"] || extension_id)
  end

  defp put_contract_refs(manifest, tool_pack) do
    refs =
      [@base_contract_ref]
      |> Kernel.++(Map.get(@tool_pack_contract_refs, tool_pack, []))
      |> Enum.uniq()

    Map.put(manifest, "contract_refs", refs)
  end

  defp normalize_errors(errors) when is_list(errors) do
    Enum.map(errors, fn
      %{"reason_code" => _code} = error ->
        stringify_keys(error)

      %{reason_code: _code} = error ->
        stringify_keys(error)

      message when is_binary(message) ->
        invalid_schema("manifest", message)

      other ->
        invalid_schema("manifest", "manifest validation error: #{inspect(other)}")
    end)
  end

  defp emit_validation_outcome(outcome, manifest, errors) do
    manifest = stringify_keys(manifest || %{})

    metadata = %{
      outcome: outcome,
      tool_pack: manifest["tool_pack"] || "unknown",
      extension_id: manifest["extension_id"] || manifest["integration_id"] || "unknown",
      reason_code: primary_reason_code(errors),
      error_count: length(errors)
    }

    Events.emit(
      [:openagents_runtime, :tools, :extensions, :manifest_validation],
      %{count: 1},
      metadata
    )
  end

  defp primary_reason_code([%{"reason_code" => reason_code} | _]), do: reason_code
  defp primary_reason_code(_), do: "policy_allowed.default"

  defp invalid_schema(path, message) do
    %{
      "reason_code" => "manifest_validation.invalid_schema",
      "path" => path,
      "message" => message
    }
  end

  defp stringify_keys(nil), do: %{}

  defp stringify_keys(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end
end
