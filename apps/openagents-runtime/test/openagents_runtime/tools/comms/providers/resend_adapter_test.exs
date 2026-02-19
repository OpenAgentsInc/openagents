defmodule OpenAgentsRuntime.Tools.Comms.Providers.ResendAdapterTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Comms.Providers.ResendAdapter

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
