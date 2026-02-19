defmodule OpenAgentsRuntimeWeb.ToolsControllerTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: false

  defmodule SuccessAdapter do
    @behaviour OpenAgentsRuntime.Tools.Coding.ProviderAdapter

    @impl true
    def execute(_request, _manifest, _opts) do
      {:ok, %{"state" => "succeeded", "data" => %{"id" => 1747}}}
    end
  end

  setup do
    previous = Application.get_env(:openagents_runtime, :runtime_tools_coding_adapter)
    Application.put_env(:openagents_runtime, :runtime_tools_coding_adapter, SuccessAdapter)

    on_exit(fn ->
      if is_nil(previous) do
        Application.delete_env(:openagents_runtime, :runtime_tools_coding_adapter)
      else
        Application.put_env(:openagents_runtime, :runtime_tools_coding_adapter, previous)
      end
    end)

    :ok
  end

  test "execute dispatches coding.v1 execute mode", %{conn: conn} do
    payload =
      base_payload()
      |> Map.put("run_id", "run_tools_1")
      |> Map.put("thread_id", "thread_tools_1")

    conn =
      conn
      |> put_internal_auth(run_id: "run_tools_1", thread_id: "thread_tools_1", user_id: 77)
      |> post(~p"/internal/v1/tools/execute", payload)

    assert %{
             "data" => %{
               "state" => "succeeded",
               "decision" => "allowed",
               "reason_code" => "policy_allowed.default",
               "provider_result" => %{"data" => %{"id" => 1747}}
             }
           } = json_response(conn, 200)
  end

  test "execute dispatches coding.v1 replay mode", %{conn: conn} do
    payload =
      base_payload()
      |> Map.put("mode", "replay")
      |> Map.put("run_id", "run_tools_2")
      |> Map.put("thread_id", "thread_tools_2")

    conn =
      conn
      |> put_internal_auth(run_id: "run_tools_2", thread_id: "thread_tools_2", user_id: 77)
      |> post(~p"/internal/v1/tools/execute", payload)

    assert %{
             "data" => %{
               "decision" => "allowed",
               "reason_code" => "policy_allowed.default",
               "replay_hash" => replay_hash,
               "evaluation_hash" => evaluation_hash
             }
           } = json_response(conn, 200)

    assert is_binary(replay_hash) and byte_size(replay_hash) == 64
    assert is_binary(evaluation_hash) and byte_size(evaluation_hash) == 64
  end

  test "execute requires x-oa-user-id header", %{conn: conn} do
    payload =
      base_payload()
      |> Map.put("run_id", "run_tools_3")
      |> Map.put("thread_id", "thread_tools_3")

    conn =
      conn
      |> put_internal_auth(run_id: "run_tools_3", thread_id: "thread_tools_3")
      |> post(~p"/internal/v1/tools/execute", payload)

    assert %{
             "error" => %{
               "code" => "invalid_request",
               "details" => ["x-oa-user-id header is required"]
             }
           } = json_response(conn, 400)
  end

  test "execute rejects mismatched user context", %{conn: conn} do
    payload =
      base_payload()
      |> Map.put("run_id", "run_tools_4")
      |> Map.put("thread_id", "thread_tools_4")
      |> Map.put("user_id", 999)

    conn =
      conn
      |> put_internal_auth(run_id: "run_tools_4", thread_id: "thread_tools_4", user_id: 77)
      |> post(~p"/internal/v1/tools/execute", payload)

    assert %{"error" => %{"code" => "forbidden"}} = json_response(conn, 403)
  end

  test "execute returns machine-readable manifest validation details", %{conn: conn} do
    payload =
      base_payload()
      |> Map.put("run_id", "run_tools_5")
      |> Map.put("thread_id", "thread_tools_5")
      |> put_in(["manifest"], invalid_manifest())

    conn =
      conn
      |> put_internal_auth(run_id: "run_tools_5", thread_id: "thread_tools_5", user_id: 77)
      |> post(~p"/internal/v1/tools/execute", payload)

    assert %{"error" => %{"code" => "invalid_request", "details" => details}} =
             json_response(conn, 422)

    assert is_list(details)
    assert Enum.any?(details, &(&1["reason_code"] == "manifest_validation.invalid_schema"))
  end

  test "execute rejects unsupported tool_pack", %{conn: conn} do
    payload =
      base_payload()
      |> Map.put("tool_pack", "comms.v1")
      |> Map.put("run_id", "run_tools_6")
      |> Map.put("thread_id", "thread_tools_6")

    conn =
      conn
      |> put_internal_auth(run_id: "run_tools_6", thread_id: "thread_tools_6", user_id: 77)
      |> post(~p"/internal/v1/tools/execute", payload)

    assert %{"error" => %{"code" => "invalid_request"}} = json_response(conn, 400)
  end

  defp base_payload do
    %{
      "tool_pack" => "coding.v1",
      "mode" => "execute",
      "manifest" => valid_manifest(),
      "request" => %{
        "integration_id" => "github.primary",
        "operation" => "get_issue",
        "repository" => "OpenAgentsInc/openagents",
        "issue_number" => 1747
      },
      "policy" => %{
        "authorization_id" => "auth_123",
        "authorization_mode" => "delegated_budget"
      }
    }
  end

  defp valid_manifest do
    %{
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
    }
  end

  defp invalid_manifest do
    valid_manifest() |> Map.delete("manifest_version")
  end
end
