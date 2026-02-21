defmodule OpenAgentsRuntime.Tools.Extensions.CodingManifestValidator do
  @moduledoc """
  Validates coding integration manifests for runtime GitHub coding operations.

  Contract references:
  - docs/protocol/extensions/extension-manifest.schema.v1.json
  - docs/protocol/coding/integration-manifest.schema.v1.json
  - docs/protocol/coding/tool-pack-contract.v1.json
  """

  @manifest_version "coding.integration.v1"
  @tool_pack "coding.v1"
  @allowed_providers MapSet.new(["github"])
  @allowed_statuses MapSet.new(["active", "inactive", "error"])

  @allowed_capabilities MapSet.new([
                          "get_issue",
                          "get_pull_request",
                          "add_issue_comment"
                        ])

  @required_capabilities MapSet.new(["get_issue", "get_pull_request"])

  @allowed_secret_providers MapSet.new(["laravel"])
  @allowed_write_modes MapSet.new(["enforce", "audit_only"])

  @type validation_error :: String.t()
  @type manifest_map :: %{optional(String.t()) => term()}

  @spec validate(map()) :: {:ok, manifest_map()} | {:error, [validation_error()]}
  def validate(manifest) when is_map(manifest) do
    manifest = stringify_keys(manifest)

    errors =
      []
      |> validate_manifest_version(manifest)
      |> validate_integration_id(manifest)
      |> validate_provider(manifest)
      |> validate_status(manifest)
      |> validate_tool_pack(manifest)
      |> validate_capabilities(manifest)
      |> validate_secrets_ref(manifest)
      |> validate_policy(manifest)
      |> Enum.reverse()

    if errors == [] do
      {:ok, normalize_manifest(manifest)}
    else
      {:error, errors}
    end
  end

  def validate(_), do: {:error, ["manifest must be an object"]}

  defp validate_manifest_version(errors, manifest) do
    if manifest["manifest_version"] == @manifest_version do
      errors
    else
      ["manifest_version must equal #{@manifest_version}" | errors]
    end
  end

  defp validate_integration_id(errors, manifest) do
    integration_id = manifest["integration_id"]

    cond do
      not is_binary(integration_id) ->
        ["integration_id must be a string" | errors]

      String.length(integration_id) < 3 or String.length(integration_id) > 128 ->
        ["integration_id length must be between 3 and 128" | errors]

      not String.match?(integration_id, ~r/^[a-zA-Z0-9._:-]+$/) ->
        ["integration_id contains invalid characters" | errors]

      true ->
        errors
    end
  end

  defp validate_provider(errors, manifest) do
    provider = manifest["provider"]

    cond do
      not is_binary(provider) ->
        ["provider must be a string" | errors]

      not MapSet.member?(@allowed_providers, provider) ->
        ["provider is not supported" | errors]

      true ->
        errors
    end
  end

  defp validate_status(errors, manifest) do
    status = manifest["status"]

    cond do
      not is_binary(status) ->
        ["status must be a string" | errors]

      not MapSet.member?(@allowed_statuses, status) ->
        ["status is not allowed" | errors]

      true ->
        errors
    end
  end

  defp validate_tool_pack(errors, manifest) do
    if manifest["tool_pack"] == @tool_pack do
      errors
    else
      ["tool_pack must equal #{@tool_pack}" | errors]
    end
  end

  defp validate_capabilities(errors, manifest) do
    capabilities = manifest["capabilities"]

    cond do
      not is_list(capabilities) ->
        ["capabilities must be an array" | errors]

      capabilities == [] ->
        ["capabilities must include at least one capability" | errors]

      true ->
        unique_capabilities = dedupe_preserving_order(capabilities)

        errors =
          if unique_capabilities != capabilities do
            ["capabilities must be unique" | errors]
          else
            errors
          end

        errors =
          Enum.reduce(unique_capabilities, errors, fn capability, acc ->
            cond do
              not is_binary(capability) ->
                ["capabilities entries must be strings" | acc]

              not MapSet.member?(@allowed_capabilities, capability) ->
                ["capability not allowed: #{inspect(capability)}" | acc]

              true ->
                acc
            end
          end)

        capability_set =
          unique_capabilities
          |> Enum.filter(&is_binary/1)
          |> MapSet.new()

        if MapSet.subset?(@required_capabilities, capability_set) do
          errors
        else
          ["capabilities must include get_issue and get_pull_request" | errors]
        end
    end
  end

  defp validate_secrets_ref(errors, manifest) do
    case stringify_keys(manifest["secrets_ref"]) do
      nil ->
        ["secrets_ref is required" | errors]

      secrets_ref when is_map(secrets_ref) ->
        errors
        |> validate_secret_provider(secrets_ref)
        |> validate_secret_key_id(secrets_ref)

      _ ->
        ["secrets_ref must be an object" | errors]
    end
  end

  defp validate_secret_provider(errors, secrets_ref) do
    provider = secrets_ref["provider"]

    cond do
      not is_binary(provider) ->
        ["secrets_ref.provider must be a string" | errors]

      not MapSet.member?(@allowed_secret_providers, provider) ->
        ["secrets_ref.provider is not allowed" | errors]

      true ->
        errors
    end
  end

  defp validate_secret_key_id(errors, secrets_ref) do
    key_id = secrets_ref["key_id"]

    cond do
      not is_binary(key_id) ->
        ["secrets_ref.key_id must be a string" | errors]

      String.length(String.trim(key_id)) < 3 ->
        ["secrets_ref.key_id must be at least 3 chars" | errors]

      String.length(key_id) > 256 ->
        ["secrets_ref.key_id must be <= 256 chars" | errors]

      true ->
        errors
    end
  end

  defp validate_policy(errors, manifest) do
    case stringify_keys(manifest["policy"]) do
      nil ->
        ["policy is required" | errors]

      policy when is_map(policy) ->
        errors
        |> validate_policy_write_mode(policy)
        |> validate_policy_rate_limit(policy)
        |> validate_policy_default_repository(policy)

      _ ->
        ["policy must be an object" | errors]
    end
  end

  defp validate_policy_write_mode(errors, policy) do
    write_mode = policy["write_operations_mode"]

    cond do
      not is_binary(write_mode) ->
        ["policy.write_operations_mode must be a string" | errors]

      not MapSet.member?(@allowed_write_modes, write_mode) ->
        ["policy.write_operations_mode is not allowed" | errors]

      true ->
        errors
    end
  end

  defp validate_policy_rate_limit(errors, policy) do
    max_rate = policy["max_requests_per_minute"]

    cond do
      not is_integer(max_rate) ->
        ["policy.max_requests_per_minute must be an integer" | errors]

      max_rate < 1 or max_rate > 10_000 ->
        ["policy.max_requests_per_minute must be between 1 and 10000" | errors]

      true ->
        errors
    end
  end

  defp validate_policy_default_repository(errors, policy) do
    default_repository = policy["default_repository"]

    cond do
      is_nil(default_repository) ->
        errors

      is_binary(default_repository) and valid_repository?(default_repository) ->
        errors

      true ->
        ["policy.default_repository must be in 'owner/repo' format when provided" | errors]
    end
  end

  defp normalize_manifest(manifest) do
    manifest
    |> Map.put("capabilities", dedupe_preserving_order(List.wrap(manifest["capabilities"])))
    |> Map.update("secrets_ref", %{}, &stringify_keys/1)
    |> Map.update("policy", %{}, &stringify_keys/1)
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

  defp valid_repository?(repository) when is_binary(repository) do
    String.match?(repository, ~r/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/)
  end

  defp valid_repository?(_), do: false
end
