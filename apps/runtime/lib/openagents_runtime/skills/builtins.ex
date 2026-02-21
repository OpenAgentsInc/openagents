defmodule OpenAgentsRuntime.Skills.Builtins do
  @moduledoc """
  Built-in ToolSpec/SkillSpec catalog for first-party integrations.

  These entries guarantee that existing `github.primary` and `resend.primary`
  capabilities are available even before registry rows are inserted.
  """

  alias OpenAgentsRuntime.DS.Receipts

  @type spec_map :: %{required(String.t()) => term()}

  @spec tool_specs() :: [spec_map()]
  def tool_specs do
    [github_tool_spec(), resend_tool_spec()]
  end

  @spec skill_specs() :: [spec_map()]
  def skill_specs do
    [github_skill_spec(), resend_skill_spec()]
  end

  @spec find_tool_spec(String.t(), pos_integer() | nil) :: spec_map() | nil
  def find_tool_spec(tool_id, version \\ nil)

  def find_tool_spec(tool_id, version) when is_binary(tool_id) do
    Enum.find(tool_specs(), fn spec ->
      spec["tool_id"] == tool_id and (is_nil(version) or spec["version"] == version)
    end)
  end

  @spec find_tool_spec_by_integration(String.t(), String.t(), pos_integer() | nil) ::
          spec_map() | nil
  def find_tool_spec_by_integration(tool_pack, integration_id, version \\ nil)

  def find_tool_spec_by_integration(tool_pack, integration_id, version)
      when is_binary(tool_pack) and is_binary(integration_id) do
    Enum.find(tool_specs(), fn spec ->
      manifest = spec["integration_manifest"] || %{}

      spec["tool_pack"] == tool_pack and
        manifest["integration_id"] == integration_id and
        (is_nil(version) or spec["version"] == version)
    end)
  end

  @spec find_skill_spec(String.t(), pos_integer() | nil) :: spec_map() | nil
  def find_skill_spec(skill_id, version \\ nil)

  def find_skill_spec(skill_id, version) when is_binary(skill_id) do
    Enum.find(skill_specs(), fn spec ->
      spec["skill_id"] == skill_id and (is_nil(version) or spec["version"] == version)
    end)
  end

  defp github_tool_spec do
    base = %{
      "tool_id" => "github.primary",
      "version" => 1,
      "tool_pack" => "coding.v1",
      "name" => "GitHub Coding",
      "description" => "Read and comment on GitHub issues/PRs through runtime policy gates.",
      "execution_kind" => "http",
      "input_schema" => %{
        "type" => "object",
        "required" => ["operation", "repository"],
        "properties" => %{
          "operation" => %{"type" => "string"},
          "repository" => %{"type" => "string"}
        }
      },
      "output_schema" => %{"type" => "object"},
      "integration_manifest" => %{
        "manifest_version" => "coding.integration.v1",
        "integration_id" => "github.primary",
        "provider" => "github",
        "status" => "active",
        "tool_pack" => "coding.v1",
        "capabilities" => ["get_issue", "get_pull_request", "add_issue_comment"],
        "secrets_ref" => %{"provider" => "laravel", "key_id" => "intsec_github_1"},
        "policy" => %{
          "write_operations_mode" => "enforce",
          "max_requests_per_minute" => 240,
          "default_repository" => "OpenAgentsInc/openagents"
        }
      },
      "auth_requirements" => %{
        "secret_ref_provider" => "laravel",
        "scopes" => ["repo:read", "issues:write"]
      },
      "safety_policy" => %{
        "timeout_ms" => 15_000,
        "network_allowlist" => ["api.github.com"]
      },
      "commercial" => %{
        "pricing_model" => "free",
        "currency" => "BTC_SATS",
        "settlement_mode" => "prepaid"
      },
      "metadata" => %{"source" => "builtin", "integration" => "github"},
      "state" => "published",
      "submitted_by" => "openagents"
    }

    Map.put(base, "content_hash", spec_hash(base))
  end

  defp resend_tool_spec do
    base = %{
      "tool_id" => "resend.primary",
      "version" => 1,
      "tool_pack" => "comms.v1",
      "name" => "Resend Comms",
      "description" => "Send and track lifecycle delivery events through Resend.",
      "execution_kind" => "http",
      "input_schema" => %{
        "type" => "object",
        "required" => ["operation"],
        "properties" => %{"operation" => %{"type" => "string"}}
      },
      "output_schema" => %{"type" => "object"},
      "integration_manifest" => %{
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
        "webhook" => %{
          "verification" => "hmac_sha256",
          "events" => ["delivered", "bounced", "complained", "unsubscribed"]
        }
      },
      "auth_requirements" => %{
        "secret_ref_provider" => "laravel",
        "scopes" => ["email:send"]
      },
      "safety_policy" => %{
        "timeout_ms" => 10_000,
        "network_allowlist" => ["api.resend.com"]
      },
      "commercial" => %{
        "pricing_model" => "free",
        "currency" => "BTC_SATS",
        "settlement_mode" => "prepaid"
      },
      "metadata" => %{"source" => "builtin", "integration" => "resend"},
      "state" => "published",
      "submitted_by" => "openagents"
    }

    Map.put(base, "content_hash", spec_hash(base))
  end

  defp github_skill_spec do
    base = %{
      "skill_id" => "github-coding",
      "version" => 1,
      "name" => "GitHub Coding Skill",
      "description" =>
        "Plan coding actions, inspect issues, and post approved comments via GitHub.",
      "license" => "Apache-2.0",
      "compatibility" => %{"runtime" => "runtime", "tool_pack" => "coding.v1"},
      "instructions_markdown" => "Use GitHub tools for repository issue and PR workflows.",
      "allowed_tools" => [%{"tool_id" => "github.primary", "version" => 1}],
      "scripts" => [],
      "references" => [],
      "assets" => [],
      "commercial" => %{"pricing_model" => "free", "currency" => "BTC_SATS"},
      "metadata" => %{"source" => "builtin", "category" => "coding"},
      "state" => "published",
      "submitted_by" => "openagents"
    }

    Map.put(base, "content_hash", spec_hash(base))
  end

  defp resend_skill_spec do
    base = %{
      "skill_id" => "resend-comms",
      "version" => 1,
      "name" => "Resend Comms Skill",
      "description" => "Execute outbound email actions with delivery-state awareness.",
      "license" => "Apache-2.0",
      "compatibility" => %{"runtime" => "runtime", "tool_pack" => "comms.v1"},
      "instructions_markdown" =>
        "Use Resend tool-pack for outbound messaging and webhook ingestion.",
      "allowed_tools" => [%{"tool_id" => "resend.primary", "version" => 1}],
      "scripts" => [],
      "references" => [],
      "assets" => [],
      "commercial" => %{"pricing_model" => "free", "currency" => "BTC_SATS"},
      "metadata" => %{"source" => "builtin", "category" => "comms"},
      "state" => "published",
      "submitted_by" => "openagents"
    }

    Map.put(base, "content_hash", spec_hash(base))
  end

  defp spec_hash(spec) do
    spec
    |> Map.delete("content_hash")
    |> Receipts.stable_hash()
  end
end
