defmodule OpenAgentsRuntime.Contracts.PolicyReasonContractTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.DS.PolicyReasonCodes

  @repo_root Path.expand("../../../../..", __DIR__)
  @json_path Path.join(@repo_root, "docs/protocol/reasons/runtime-policy-reason-codes.v1.json")
  @proto_path Path.join(@repo_root, "proto/openagents/protocol/v1/reasons.proto")

  test "runtime reason taxonomy JSON, proto enum, and runtime module are converged" do
    json = load_json!(@json_path)

    json_codes =
      json["reason_codes"]
      |> List.wrap()
      |> Enum.map(& &1["code"])
      |> Enum.reject(&is_nil/1)
      |> MapSet.new()

    json_domains =
      json["domains"]
      |> List.wrap()
      |> Enum.reject(&is_nil/1)
      |> Enum.sort_by(&String.length/1, :desc)

    proto_codes =
      @proto_path
      |> File.read!()
      |> extract_reason_enum_names()
      |> Enum.reject(&(&1 == "REASON_CODE_UNSPECIFIED"))
      |> Enum.map(&enum_name_to_reason_code(&1, json_domains))
      |> MapSet.new()

    runtime_codes = PolicyReasonCodes.all() |> MapSet.new()

    assert MapSet.equal?(json_codes, runtime_codes),
           mismatch_message("runtime module vs canonical JSON", json_codes, runtime_codes)

    assert MapSet.equal?(json_codes, proto_codes),
           mismatch_message("proto enum vs canonical JSON", json_codes, proto_codes)
  end

  defp load_json!(path) do
    path
    |> File.read!()
    |> Jason.decode!()
  end

  defp extract_reason_enum_names(proto_body) when is_binary(proto_body) do
    Regex.scan(~r/^\s*(REASON_CODE_[A-Z0-9_]+)\s*=\s*\d+\s*;/m, proto_body)
    |> Enum.map(fn [_full, name] -> name end)
  end

  defp enum_name_to_reason_code(name, json_domains) when is_binary(name) do
    normalized = name |> String.replace_prefix("REASON_CODE_", "") |> String.downcase()

    case Enum.find(json_domains, &String.starts_with?(normalized, &1 <> "_")) do
      nil ->
        raise """
        unable to map proto enum '#{name}' to a canonical reason code domain.
        expected one of domains: #{Enum.join(json_domains, ", ")}
        """

      domain ->
        remainder = String.replace_prefix(normalized, domain <> "_", "")
        domain <> "." <> remainder
    end
  end

  defp mismatch_message(label, expected_set, actual_set) do
    missing = MapSet.difference(expected_set, actual_set) |> MapSet.to_list() |> Enum.sort()
    extra = MapSet.difference(actual_set, expected_set) |> MapSet.to_list() |> Enum.sort()

    """
    #{label} mismatch
    missing: #{inspect(missing)}
    extra: #{inspect(extra)}
    """
  end
end
