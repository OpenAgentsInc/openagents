defmodule OpenAgentsRuntime.Tools.Extensions.CodingManifestValidatorTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Extensions.CodingManifestValidator

  test "validate/1 accepts a valid github coding manifest and normalizes arrays" do
    manifest = %{
      manifest_version: "coding.integration.v1",
      integration_id: "github.primary",
      provider: "github",
      status: "active",
      tool_pack: "coding.v1",
      capabilities: ["get_issue", "get_pull_request", "add_issue_comment"],
      secrets_ref: %{provider: "laravel", key_id: "intsec_github_1"},
      policy: %{
        write_operations_mode: "enforce",
        max_requests_per_minute: 120,
        default_repository: "OpenAgentsInc/openagents"
      }
    }

    assert {:ok, normalized} = CodingManifestValidator.validate(manifest)

    assert normalized["manifest_version"] == "coding.integration.v1"
    assert normalized["tool_pack"] == "coding.v1"

    assert normalized["capabilities"] == [
             "get_issue",
             "get_pull_request",
             "add_issue_comment"
           ]

    assert normalized["policy"]["default_repository"] == "OpenAgentsInc/openagents"
  end

  test "validate/1 rejects missing required sections" do
    manifest = %{
      manifest_version: "coding.integration.v1",
      integration_id: "github.primary",
      provider: "github",
      status: "active",
      tool_pack: "coding.v1",
      capabilities: ["get_issue", "get_pull_request"]
    }

    assert {:error, errors} = CodingManifestValidator.validate(manifest)

    assert "secrets_ref is required" in errors
    assert "policy is required" in errors
  end

  test "validate/1 rejects unsupported provider/capability/write mode values" do
    manifest = %{
      manifest_version: "coding.integration.v1",
      integration_id: "github.primary",
      provider: "gitlab",
      status: "active",
      tool_pack: "coding.v1",
      capabilities: ["get_issue", "invalid_capability"],
      secrets_ref: %{provider: "runtime", key_id: "intsec_github_1"},
      policy: %{write_operations_mode: "unsafe", max_requests_per_minute: 120}
    }

    assert {:error, errors} = CodingManifestValidator.validate(manifest)

    assert "provider is not supported" in errors
    assert "capability not allowed: \"invalid_capability\"" in errors
    assert "secrets_ref.provider is not allowed" in errors
    assert "policy.write_operations_mode is not allowed" in errors
  end

  test "validate/1 rejects invalid policy rate limit and missing required capabilities" do
    manifest = %{
      manifest_version: "coding.integration.v1",
      integration_id: "github.primary",
      provider: "github",
      status: "active",
      tool_pack: "coding.v1",
      capabilities: ["add_issue_comment"],
      secrets_ref: %{provider: "laravel", key_id: "intsec_github_1"},
      policy: %{write_operations_mode: "enforce", max_requests_per_minute: 0}
    }

    assert {:error, errors} = CodingManifestValidator.validate(manifest)

    assert "capabilities must include get_issue and get_pull_request" in errors
    assert "policy.max_requests_per_minute must be between 1 and 10000" in errors
  end
end
