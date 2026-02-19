defmodule OpenAgentsRuntime.Tools.Comms.Providers.ResendAdapterTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Comms.Providers.ResendAdapter

  defmodule SecretClientStub do
    def fetch_secret("resend", scope, opts) do
      send(self(), {:secret_fetch_scope, scope, opts})
      {:ok, "re_scoped_key"}
    end
  end

  defmodule SecretClientFailureStub do
    def fetch_secret("resend", _scope, _opts), do: {:error, :transport_error}
  end

  test "send/3 maps successful Resend responses to sent state" do
    http_client = fn endpoint, api_key, payload, timeout_ms ->
      send(self(), {:http_called, endpoint, api_key, payload, timeout_ms})
      {:ok, 200, ~s({"id":"email_123"})}
    end

    request = base_request()

    assert {:ok, result} =
             ResendAdapter.send(request, %{},
               api_key: "re_test_key",
               from: "noreply@example.com",
               timeout_ms: 5_000,
               http_client: http_client
             )

    assert_receive {:http_called, endpoint, api_key, payload, timeout_ms}
    assert endpoint == "https://api.resend.com/emails"
    assert api_key == "re_test_key"
    assert timeout_ms == 5_000
    assert payload["from"] == "noreply@example.com"
    assert payload["to"] == ["user@example.com"]
    assert payload["subject"] == "OpenAgents message (welcome_email)"

    assert result["state"] == "sent"
    assert result["reason_code"] == "policy_allowed.default"
    assert result["message_id"] == "email_123"
    assert result["provider_status"] == 200
  end

  test "send/3 fetches API key from scoped runtime secret client when direct key is absent" do
    http_client = fn endpoint, api_key, payload, timeout_ms ->
      send(self(), {:http_called, endpoint, api_key, payload, timeout_ms})
      {:ok, 200, ~s({"id":"email_from_scoped_secret"})}
    end

    request =
      base_request()
      |> Map.put("user_id", 42)
      |> Map.put("run_id", "run_1")
      |> Map.put("tool_call_id", "tool_1")

    assert {:ok, result} =
             ResendAdapter.send(request, %{},
               from: "noreply@example.com",
               secret_client: SecretClientStub,
               secret_client_opts: [request_timeout_ms: 1500],
               http_client: http_client
             )

    assert_receive {:secret_fetch_scope, scope, secret_opts}
    assert scope["user_id"] == 42
    assert scope["integration_id"] == "resend.primary"
    assert scope["run_id"] == "run_1"
    assert scope["tool_call_id"] == "tool_1"
    assert secret_opts == [request_timeout_ms: 1500]

    assert_receive {:http_called, endpoint, api_key, payload, timeout_ms}
    assert endpoint == "https://api.resend.com/emails"
    assert api_key == "re_scoped_key"
    assert timeout_ms == 10_000
    assert payload["from"] == "noreply@example.com"
    tags = payload["tags"] || []
    assert Enum.any?(tags, &(&1["name"] == "user_id" and &1["value"] == "42"))
    assert Enum.any?(tags, &(&1["name"] == "run_id" and &1["value"] == "run_1"))
    assert Enum.any?(tags, &(&1["name"] == "tool_call_id" and &1["value"] == "tool_1"))
    assert result["state"] == "sent"
    assert result["message_id"] == "email_from_scoped_secret"
  end

  test "send/3 maps scoped secret fetch transport failures to provider error reason" do
    request =
      base_request()
      |> Map.put("user_id", 42)
      |> Map.put("run_id", "run_1")
      |> Map.put("tool_call_id", "tool_1")

    assert {:error, error} =
             ResendAdapter.send(request, %{},
               from: "noreply@example.com",
               secret_client: SecretClientFailureStub
             )

    assert error["reason_code"] == "comms_failed.provider_error"
    assert error["message"] == "runtime_secret_fetch_failed:transport_error"
  end

  test "send/3 maps 401/403 to explicit deny reason" do
    for status <- [401, 403] do
      http_client = fn _endpoint, _api_key, _payload, _timeout_ms ->
        {:ok, status, ~s({"message":"unauthorized"})}
      end

      assert {:error, error} =
               ResendAdapter.send(base_request(), %{},
                 api_key: "re_test_key",
                 from: "noreply@example.com",
                 http_client: http_client
               )

      assert error["reason_code"] == "policy_denied.explicit_deny"
      assert error["provider_status"] == status
    end
  end

  test "send/3 maps 429 to budget exhausted and 422 to invalid schema" do
    rate_client = fn _endpoint, _api_key, _payload, _timeout_ms ->
      {:ok, 429, ~s({"message":"rate limited"})}
    end

    schema_client = fn _endpoint, _api_key, _payload, _timeout_ms ->
      {:ok, 422, ~s({"message":"invalid payload"})}
    end

    assert {:error, rate_error} =
             ResendAdapter.send(base_request(), %{},
               api_key: "re_test_key",
               from: "noreply@example.com",
               http_client: rate_client
             )

    assert rate_error["reason_code"] == "policy_denied.budget_exhausted"
    assert rate_error["provider_status"] == 429

    assert {:error, schema_error} =
             ResendAdapter.send(base_request(), %{},
               api_key: "re_test_key",
               from: "noreply@example.com",
               http_client: schema_client
             )

    assert schema_error["reason_code"] == "manifest_validation.invalid_schema"
    assert schema_error["provider_status"] == 422
  end

  test "send/3 maps transport failures to provider error reason" do
    failing_client = fn _endpoint, _api_key, _payload, _timeout_ms ->
      {:error, {:transport, :econnrefused}}
    end

    assert {:error, error} =
             ResendAdapter.send(base_request(), %{},
               api_key: "re_test_key",
               from: "noreply@example.com",
               http_client: failing_client
             )

    assert error["reason_code"] == "comms_failed.provider_error"
    assert error["provider_status"] == nil
  end

  test "send/3 blocks private endpoint targets through guarded network seam" do
    assert {:error, error} =
             ResendAdapter.send(base_request(), %{},
               api_key: "re_test_key",
               from: "noreply@example.com",
               endpoint: "http://127.0.0.1:4000/emails"
             )

    assert error["reason_code"] == "ssrf_block.private_address"
    assert error["ssrf_block_reason"] == "ssrf_block.private_address"
  end

  test "send/3 blocks non-allowlisted endpoint hosts through guarded network seam" do
    dns_resolver = fn "example.com" -> {:ok, [{8, 8, 8, 8}]} end

    assert {:error, error} =
             ResendAdapter.send(base_request(), %{},
               api_key: "re_test_key",
               from: "noreply@example.com",
               endpoint: "https://example.com/emails",
               dns_resolver: dns_resolver,
               allowed_hosts: ["api.resend.com"]
             )

    assert error["reason_code"] == "ssrf_block.host_not_allowed"
    assert error["ssrf_block_reason"] == "ssrf_block.host_not_allowed"
  end

  test "send/3 rejects missing api key and invalid payloads" do
    assert {:error, error_missing_key} =
             ResendAdapter.send(base_request(), %{}, from: "noreply@example.com")

    assert error_missing_key["reason_code"] == "policy_denied.explicit_deny"

    invalid_request = Map.delete(base_request(), "recipient")

    assert {:error, invalid_payload} =
             ResendAdapter.send(invalid_request, %{},
               api_key: "re_test_key",
               from: "noreply@example.com"
             )

    assert invalid_payload["reason_code"] == "manifest_validation.invalid_schema"
  end

  defp base_request do
    %{
      "integration_id" => "resend.primary",
      "recipient" => "user@example.com",
      "template_id" => "welcome_email",
      "variables" => %{"first_name" => "Casey"}
    }
  end
end
