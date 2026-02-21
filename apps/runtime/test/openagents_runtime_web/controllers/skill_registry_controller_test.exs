defmodule OpenAgentsRuntimeWeb.SkillRegistryControllerTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: false

  test "list_tool_specs includes built-in github tool", %{conn: conn} do
    conn =
      conn
      |> put_internal_auth(user_id: 42)
      |> get(~p"/internal/v1/skills/tool-specs")

    assert %{"data" => tool_specs} = json_response(conn, 200)
    assert Enum.any?(tool_specs, &(&1["tool_id"] == "github.primary"))
  end

  test "upsert_tool_spec validates user header and creates row", %{conn: conn} do
    tool_id = unique_id("tool")

    payload = %{
      "tool_spec" => %{
        "tool_id" => tool_id,
        "version" => 1,
        "tool_pack" => "coding.v1",
        "name" => "Runtime Tool",
        "description" => "Tool registry submission",
        "execution_kind" => "http",
        "integration_manifest" => valid_coding_manifest(tool_id),
        "commercial" => %{"pricing_model" => "free", "currency" => "BTC_SATS"}
      }
    }

    conn =
      conn
      |> put_internal_auth(user_id: 42)
      |> post(~p"/internal/v1/skills/tool-specs", payload)

    assert %{"data" => %{"tool_id" => ^tool_id, "version" => 1, "state" => "validated"}} =
             json_response(conn, 201)
  end

  test "upsert_skill_spec then publish returns release", %{conn: conn} do
    tool_id = unique_id("tool")
    skill_id = unique_id("skill")

    tool_payload = %{
      "tool_spec" => %{
        "tool_id" => tool_id,
        "version" => 1,
        "tool_pack" => "coding.v1",
        "name" => "Runtime Tool",
        "description" => "Tool registry submission",
        "execution_kind" => "http",
        "integration_manifest" => valid_coding_manifest(tool_id),
        "commercial" => %{"pricing_model" => "free", "currency" => "BTC_SATS"}
      },
      "state" => "published"
    }

    conn
    |> put_internal_auth(user_id: 42)
    |> post(~p"/internal/v1/skills/tool-specs", tool_payload)
    |> json_response(201)

    skill_payload = %{
      "skill_spec" => %{
        "skill_id" => skill_id,
        "version" => 1,
        "name" => "Registry Skill",
        "description" => "skill for controller test",
        "instructions_markdown" => "Use the tool for coding operations.",
        "compatibility" => %{"runtime" => "runtime"},
        "allowed_tools" => [%{"tool_id" => tool_id, "version" => 1}],
        "commercial" => %{"pricing_model" => "free", "currency" => "BTC_SATS"}
      }
    }

    conn
    |> recycle()
    |> put_internal_auth(user_id: 42)
    |> post(~p"/internal/v1/skills/skill-specs", skill_payload)
    |> json_response(201)

    publish_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 42)
      |> post(~p"/internal/v1/skills/skill-specs/#{skill_id}/1/publish", %{})

    assert %{
             "data" => %{
               "skill_id" => ^skill_id,
               "version" => 1,
               "bundle_hash" => bundle_hash
             }
           } = json_response(publish_conn, 201)

    assert is_binary(bundle_hash)

    release_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 42)
      |> get(~p"/internal/v1/skills/releases/#{skill_id}/1")

    assert %{"data" => %{"skill_id" => ^skill_id, "version" => 1, "bundle" => bundle}} =
             json_response(release_conn, 200)

    assert bundle["bundle_format"] == "agent_skills.v1"
  end

  test "upsert_tool_spec requires x-oa-user-id", %{conn: conn} do
    conn =
      conn
      |> put_internal_auth()
      |> post(~p"/internal/v1/skills/tool-specs", %{"tool_spec" => %{}})

    assert %{"error" => %{"code" => "invalid_request"}} = json_response(conn, 400)
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
