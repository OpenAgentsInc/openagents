defmodule OpenAgentsRuntime.Tools.Extensions.ManifestValidatorTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Extensions.ManifestValidator

  test "validate/1 accepts backward-compatible manifests with integration_id only" do
    manifest = %{
      "manifest_version" => "comms.integration.v1",
      "integration_id" => "resend.primary",
      "tool_pack" => "comms.v1",
      "provider" => "resend",
      "status" => "active",
      "capabilities" => ["send", "record_delivery_event"],
      "metadata" => %{"owner" => "runtime"}
    }

    assert {:ok, normalized} = ManifestValidator.validate(manifest)

    assert normalized["extension_id"] == "resend.primary"
    assert normalized["integration_id"] == "resend.primary"
    assert normalized["capabilities"] == ["send", "record_delivery_event"]
    assert normalized["metadata"]["owner"] == "runtime"
  end

  test "validate/1 accepts explicit extension_id + integration_id when matched" do
    manifest = %{
      manifest_version: "comms.integration.v1",
      extension_id: "resend.primary",
      integration_id: "resend.primary",
      tool_pack: "comms.v1",
      provider: "resend",
      status: "inactive",
      capabilities: ["send"]
    }

    assert {:ok, normalized} = ManifestValidator.validate(manifest)
    assert normalized["extension_id"] == "resend.primary"
  end

  test "validate/1 rejects mismatched extension identity aliases" do
    manifest = %{
      "manifest_version" => "comms.integration.v1",
      "extension_id" => "resend.primary",
      "integration_id" => "resend.backup",
      "tool_pack" => "comms.v1",
      "provider" => "resend",
      "status" => "active",
      "capabilities" => ["send"]
    }

    assert {:error, errors} = ManifestValidator.validate(manifest)

    assert Enum.any?(
             errors,
             &(&1["path"] == "extension_id" and
                 &1["reason_code"] == "manifest_validation.invalid_schema")
           )
  end

  test "validate/1 rejects invalid schema fields with machine-readable reason code" do
    assert {:error, errors} =
             ManifestValidator.validate(%{
               "manifest_version" => "",
               "integration_id" => "x",
               "tool_pack" => "",
               "provider" => "",
               "status" => "broken",
               "capabilities" => [123]
             })

    assert Enum.all?(errors, &(&1["reason_code"] == "manifest_validation.invalid_schema"))
    assert Enum.any?(errors, &(&1["path"] == "manifest_version"))
    assert Enum.any?(errors, &(&1["path"] == "status"))
    assert Enum.any?(errors, &(&1["path"] == "capabilities"))
  end
end
