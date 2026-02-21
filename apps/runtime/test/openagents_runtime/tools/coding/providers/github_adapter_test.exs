defmodule OpenAgentsRuntime.Tools.Coding.Providers.GitHubAdapterTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Coding.Providers.GitHubAdapter

  defmodule SecretClientStub do
    def fetch_secret("github", scope, opts) do
      send(self(), {:secret_fetch_scope, scope, opts})
      {:ok, "ghp_scoped_token"}
    end
  end

  defmodule SecretClientFailureStub do
    def fetch_secret("github", _scope, _opts), do: {:error, :transport_error}
  end

  test "execute/3 maps successful get_issue responses" do
    transport = fn :get, url, headers, _body, _opts ->
      send(self(), {:transport_called, :get, url, headers})

      {:ok, 200, [],
       ~s({"id":123,"number":42,"title":"Fix runtime bug","state":"open","html_url":"https://github.com/OpenAgentsInc/openagents/issues/42"})}
    end

    request = base_request() |> Map.put("operation", "get_issue") |> Map.put("issue_number", 42)

    assert {:ok, result} =
             GitHubAdapter.execute(request, %{},
               api_token: "ghp_test_token",
               network_transport: transport
             )

    assert_receive {:transport_called, :get, url, headers}
    assert url == "https://api.github.com/repos/OpenAgentsInc/openagents/issues/42"

    auth_header = Enum.find(headers, fn {k, _v} -> k == "authorization" end)
    assert auth_header == {"authorization", "Bearer ghp_test_token"}

    assert result["state"] == "succeeded"
    assert result["reason_code"] == "policy_allowed.default"
    assert result["data"]["number"] == 42
  end

  test "execute/3 maps successful add_issue_comment responses" do
    transport = fn :post, url, _headers, body, _opts ->
      send(self(), {:transport_called, :post, url, body})

      {:ok, 201, [],
       ~s({"id":777,"html_url":"https://github.com/OpenAgentsInc/openagents/issues/42#issuecomment-777"})}
    end

    request =
      base_request()
      |> Map.put("operation", "add_issue_comment")
      |> Map.put("issue_number", 42)
      |> Map.put("body", "Looks good to me")

    assert {:ok, result} =
             GitHubAdapter.execute(request, %{},
               api_token: "ghp_test_token",
               network_transport: transport
             )

    assert_receive {:transport_called, :post, url, encoded_body}
    assert url == "https://api.github.com/repos/OpenAgentsInc/openagents/issues/42/comments"
    assert encoded_body == ~s({"body":"Looks good to me"})

    assert result["state"] == "succeeded"
    assert result["comment_id"] == 777
    assert result["operation"] == "add_issue_comment"
  end

  test "execute/3 fetches API token from scoped runtime secret client when direct token is absent" do
    transport = fn _method, _url, _headers, _body, _opts ->
      {:ok, 200, [], ~s({"id":123,"number":42,"title":"Fix runtime bug"})}
    end

    request =
      base_request()
      |> Map.put("operation", "get_issue")
      |> Map.put("issue_number", 42)
      |> Map.put("user_id", 42)
      |> Map.put("run_id", "run_1")
      |> Map.put("tool_call_id", "tool_1")

    assert {:ok, _result} =
             GitHubAdapter.execute(request, %{},
               secret_client: SecretClientStub,
               secret_client_opts: [request_timeout_ms: 1500],
               network_transport: transport
             )

    assert_receive {:secret_fetch_scope, scope, secret_opts}
    assert scope["user_id"] == 42
    assert scope["integration_id"] == "github.primary"
    assert scope["run_id"] == "run_1"
    assert scope["tool_call_id"] == "tool_1"
    assert secret_opts == [request_timeout_ms: 1500]
  end

  test "execute/3 maps scoped secret fetch transport failures to provider error reason" do
    request =
      base_request()
      |> Map.put("operation", "get_issue")
      |> Map.put("issue_number", 42)
      |> Map.put("user_id", 42)
      |> Map.put("run_id", "run_1")
      |> Map.put("tool_call_id", "tool_1")

    assert {:error, error} =
             GitHubAdapter.execute(request, %{}, secret_client: SecretClientFailureStub)

    assert error["reason_code"] == "coding_failed.provider_error"
    assert error["message"] == "runtime_secret_fetch_failed:transport_error"
  end

  test "execute/3 maps 401/403 to explicit deny reason" do
    for status <- [401, 403] do
      transport = fn _method, _url, _headers, _body, _opts ->
        {:ok, status, [], ~s({"message":"unauthorized"})}
      end

      request =
        base_request() |> Map.put("operation", "get_issue") |> Map.put("issue_number", 42)

      assert {:error, error} =
               GitHubAdapter.execute(request, %{},
                 api_token: "ghp_test_token",
                 network_transport: transport
               )

      assert error["reason_code"] == "policy_denied.explicit_deny"
      assert error["provider_status"] == status
    end
  end

  test "execute/3 blocks private endpoint targets through guarded network seam" do
    request = base_request() |> Map.put("operation", "get_issue") |> Map.put("issue_number", 42)

    assert {:error, error} =
             GitHubAdapter.execute(request, %{},
               api_token: "ghp_test_token",
               endpoint_base: "http://127.0.0.1:8080"
             )

    assert error["reason_code"] == "ssrf_block.private_address"
    assert error["ssrf_block_reason"] == "ssrf_block.private_address"
  end

  test "execute/3 rejects missing token and invalid payloads" do
    request = base_request() |> Map.put("operation", "get_issue") |> Map.put("issue_number", 42)

    assert {:error, missing_token} = GitHubAdapter.execute(request, %{}, [])
    assert missing_token["reason_code"] == "policy_denied.explicit_deny"

    invalid =
      base_request()
      |> Map.put("operation", "add_issue_comment")
      |> Map.put("issue_number", 42)
      |> Map.delete("body")

    assert {:error, invalid_payload} =
             GitHubAdapter.execute(invalid, %{}, api_token: "ghp_test_token")

    assert invalid_payload["reason_code"] == "manifest_validation.invalid_schema"
  end

  defp base_request do
    %{
      "integration_id" => "github.primary",
      "repository" => "OpenAgentsInc/openagents"
    }
  end
end
