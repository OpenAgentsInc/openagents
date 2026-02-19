defmodule OpenAgentsRuntime.Tools.Extensions.ManifestRegistryTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Extensions.ManifestRegistry

  test "validate_for_activation/2 accepts valid comms manifest and attaches contract refs" do
    assert {:ok, manifest} = ManifestRegistry.validate_for_activation(valid_comms_manifest())

    assert manifest["extension_id"] == "resend.primary"
    assert manifest["integration_id"] == "resend.primary"

    assert "docs/protocol/extensions/extension-manifest.schema.v1.json" in manifest[
             "contract_refs"
           ]

    assert "docs/protocol/comms/integration-manifest.schema.v1.json" in manifest["contract_refs"]
  end

  test "validate_for_activation/2 rejects unsupported tool packs deterministically" do
    invalid_manifest =
      valid_comms_manifest()
      |> Map.put("tool_pack", "unknown.v1")

    assert {:error, {:invalid_manifest, [error]}} =
             ManifestRegistry.validate_for_activation(invalid_manifest)

    assert error["reason_code"] == "manifest_validation.invalid_schema"
    assert error["path"] == "tool_pack"
  end

  test "validate_for_activation/2 normalizes comms validator errors into machine-readable shape" do
    invalid_manifest =
      valid_comms_manifest()
      |> Map.put("provider", "mailgun")
      |> put_in(["webhook", "verification"], "none")

    assert {:error, {:invalid_manifest, errors}} =
             ManifestRegistry.validate_for_activation(invalid_manifest)

    assert Enum.all?(errors, &(&1["reason_code"] == "manifest_validation.invalid_schema"))
    assert Enum.any?(errors, &String.contains?(&1["message"], "provider is not supported"))

    assert Enum.any?(
             errors,
             &String.contains?(&1["message"], "webhook.verification is not allowed")
           )
  end

  test "validate_for_activation/2 emits validation outcome telemetry for rejected manifests" do
    handler_id = "manifest-registry-reject-#{System.unique_integer([:positive])}"
    parity_handler_id = "manifest-registry-parity-#{System.unique_integer([:positive])}"
    parent = self()

    :ok =
      :telemetry.attach(
        handler_id,
        [:openagents_runtime, :tools, :extensions, :manifest_validation],
        fn _event, measurements, metadata, test_pid ->
          send(test_pid, {:manifest_validation, measurements, metadata})
        end,
        parent
      )

    on_exit(fn -> :telemetry.detach(handler_id) end)

    :ok =
      :telemetry.attach(
        parity_handler_id,
        [:openagents_runtime, :parity, :failure],
        fn _event, measurements, metadata, test_pid ->
          send(test_pid, {:parity_failure, measurements, metadata})
        end,
        parent
      )

    on_exit(fn -> :telemetry.detach(parity_handler_id) end)

    invalid_manifest =
      valid_comms_manifest()
      |> Map.delete("manifest_version")

    assert {:error, {:invalid_manifest, _errors}} =
             ManifestRegistry.validate_for_activation(invalid_manifest)

    assert_receive {:manifest_validation, %{count: 1}, metadata}
    assert metadata.outcome == "rejected"
    assert metadata.reason_code == "manifest_validation.invalid_schema"

    assert_receive {:parity_failure, %{count: 1}, parity_metadata}
    assert parity_metadata.class == "manifest"
    assert parity_metadata.reason_class == "manifest_validation.invalid_schema"
  end

  test "supported_tool_packs/0 includes comms tool pack" do
    assert "comms.v1" in ManifestRegistry.supported_tool_packs()
  end

  defp valid_comms_manifest do
    %{
      "manifest_version" => "comms.integration.v1",
      "integration_id" => "resend.primary",
      "provider" => "resend",
      "status" => "active",
      "tool_pack" => "comms.v1",
      "capabilities" => ["send", "record_delivery_event"],
      "secrets_ref" => %{"provider" => "laravel", "key_id" => "intsec_resend_1"},
      "policy" => %{
        "consent_required" => true,
        "suppression_mode" => "enforce",
        "max_send_per_minute" => 120
      },
      "webhook" => %{"verification" => "hmac_sha256", "events" => ["delivered"]}
    }
  end
end
