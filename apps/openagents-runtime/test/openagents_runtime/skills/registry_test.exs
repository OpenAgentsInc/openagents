defmodule OpenAgentsRuntime.Skills.RegistryTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Skills.Registry

  test "upsert_tool_spec/2 stores validated spec" do
    attrs = %{
      "tool_id" => unique_id("tool"),
      "version" => 1,
      "tool_pack" => "coding.v1",
      "name" => "GitHub Override",
      "description" => "Runtime test tool spec",
      "execution_kind" => "http",
      "integration_manifest" => valid_coding_manifest(unique_id("github")),
      "input_schema" => %{"type" => "object"},
      "output_schema" => %{"type" => "object"},
      "commercial" => %{"pricing_model" => "free", "currency" => "BTC_SATS"},
      "metadata" => %{"test" => true}
    }

    assert {:ok, tool_spec} = Registry.upsert_tool_spec(attrs, state: "validated")
    assert tool_spec.tool_id == attrs["tool_id"]
    assert tool_spec.state == "validated"
    assert is_binary(tool_spec.content_hash)
    assert byte_size(tool_spec.content_hash) == 64
  end

  test "resolve_tool_manifest/2 resolves built-in github manifest" do
    assert {:ok, manifest} =
             Registry.resolve_tool_manifest("coding.v1", %{
               "integration_id" => "github.primary"
             })

    assert manifest["integration_id"] == "github.primary"
    assert manifest["tool_pack"] == "coding.v1"
  end

  test "publish_skill/3 compiles bundle and stores release" do
    tool_id = unique_id("tool")
    skill_id = unique_id("skill")

    assert {:ok, _tool_spec} =
             Registry.upsert_tool_spec(
               %{
                 "tool_id" => tool_id,
                 "version" => 1,
                 "tool_pack" => "coding.v1",
                 "name" => "Skill Tool",
                 "description" => "tool for compiled skill",
                 "execution_kind" => "http",
                 "integration_manifest" => valid_coding_manifest(tool_id),
                 "commercial" => %{"pricing_model" => "free", "currency" => "BTC_SATS"}
               },
               state: "published"
             )

    assert {:ok, _skill_spec} =
             Registry.upsert_skill_spec(
               %{
                 "skill_id" => skill_id,
                 "version" => 1,
                 "name" => "Registry Skill",
                 "description" => "compiled skill",
                 "instructions_markdown" => "Run coding operations safely.",
                 "compatibility" => %{"runtime" => "openagents-runtime"},
                 "allowed_tools" => [%{"tool_id" => tool_id, "version" => 1}],
                 "scripts" => [%{"path" => "scripts/run.sh", "description" => "runner"}],
                 "references" => [%{"path" => "references/readme.md"}],
                 "assets" => [%{"path" => "assets/icon.png"}],
                 "commercial" => %{"pricing_model" => "free", "currency" => "BTC_SATS"},
                 "metadata" => %{"team" => "runtime"}
               },
               state: "validated"
             )

    assert {:ok, release} = Registry.publish_skill(skill_id, 1)
    assert release.skill_id == skill_id
    assert release.version == 1
    assert release.compatibility_report["compatible"] == true

    bundle = release.bundle
    assert bundle["bundle_format"] == "agent_skills.v1"
    assert is_map(bundle["files"])

    root = bundle["root"]
    assert is_binary(root)
    assert Map.has_key?(bundle["files"], "#{root}/SKILL.md")
    assert Registry.get_skill_release(skill_id, 1).release_id == release.release_id
  end

  defp valid_coding_manifest(integration_id) do
    %{
      "manifest_version" => "coding.integration.v1",
      "integration_id" => integration_id,
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
    }
  end

  defp unique_id(prefix), do: "#{prefix}_#{System.unique_integer([:positive])}"
end
