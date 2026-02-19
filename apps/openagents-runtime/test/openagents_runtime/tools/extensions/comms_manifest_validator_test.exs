defmodule OpenAgentsRuntime.Tools.Extensions.CommsManifestValidatorTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Extensions.CommsManifestValidator

  test "validate/1 accepts a valid resend manifest and normalizes arrays" do
    manifest = %{
      manifest_version: "comms.integration.v1",
      integration_id: "resend.primary",
      provider: "resend",
      status: "active",
      tool_pack: "comms.v1",
      capabilities: ["send", "record_delivery_event"],
      secrets_ref: %{provider: "laravel", key_id: "intsec_resend_1"},
      policy: %{consent_required: true, suppression_mode: "enforce", max_send_per_minute: 60},
      webhook: %{verification: "hmac_sha256", events: ["delivered", "bounced"]}
    }

    assert {:ok, normalized} = CommsManifestValidator.validate(manifest)

    assert normalized["manifest_version"] == "comms.integration.v1"
    assert normalized["tool_pack"] == "comms.v1"
    assert normalized["capabilities"] == ["send", "record_delivery_event"]
    assert normalized["webhook"]["events"] == ["delivered", "bounced"]
  end

  test "validate/1 rejects missing required sections" do
    manifest = %{
      manifest_version: "comms.integration.v1",
      integration_id: "resend.primary",
      provider: "resend",
      status: "active",
      tool_pack: "comms.v1",
      capabilities: ["send", "record_delivery_event"]
    }

    assert {:error, errors} = CommsManifestValidator.validate(manifest)

    assert "secrets_ref is required" in errors
    assert "policy is required" in errors
    assert "webhook is required" in errors
  end

  test "validate/1 rejects unsupported provider/capability/verification values" do
    manifest = %{
      manifest_version: "comms.integration.v1",
      integration_id: "resend.primary",
      provider: "mailgun",
      status: "active",
      tool_pack: "comms.v1",
      capabilities: ["send", "unknown_capability"],
      secrets_ref: %{provider: "runtime", key_id: "intsec_resend_1"},
      policy: %{consent_required: true, suppression_mode: "bad_mode", max_send_per_minute: 60},
      webhook: %{verification: "none", events: ["delivered", "mystery"]}
    }

    assert {:error, errors} = CommsManifestValidator.validate(manifest)

    assert "provider is not supported" in errors
    assert "capability not allowed: \"unknown_capability\"" in errors
    assert "secrets_ref.provider is not allowed" in errors
    assert "policy.suppression_mode is not allowed" in errors
    assert "webhook.verification is not allowed" in errors
    assert "webhook event not allowed: \"mystery\"" in errors
  end

  test "validate/1 rejects invalid policy rate limit and missing required capabilities" do
    manifest = %{
      manifest_version: "comms.integration.v1",
      integration_id: "resend.primary",
      provider: "resend",
      status: "active",
      tool_pack: "comms.v1",
      capabilities: ["render_template"],
      secrets_ref: %{provider: "laravel", key_id: "intsec_resend_1"},
      policy: %{consent_required: true, suppression_mode: "enforce", max_send_per_minute: 0},
      webhook: %{verification: "hmac_sha256", events: ["delivered"]}
    }

    assert {:error, errors} = CommsManifestValidator.validate(manifest)

    assert "capabilities must include send and record_delivery_event" in errors
    assert "policy.max_send_per_minute must be between 1 and 10000" in errors
  end
end
