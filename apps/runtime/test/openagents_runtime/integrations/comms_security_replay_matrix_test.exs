defmodule OpenAgentsRuntime.Integrations.CommsSecurityReplayMatrixTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Integrations.LaravelSecretClient
  alias OpenAgentsRuntime.Tools.Comms.Kernel
  alias OpenAgentsRuntime.Tools.Comms.Providers.ResendAdapter
  alias OpenAgentsRuntime.Tools.ProviderCircuitBreaker

  setup do
    ProviderCircuitBreaker.reset(:all)
    LaravelSecretClient.clear_cache()
    :ok
  end

  test "INC-COMMS-201 send flow fetches scoped Laravel secret and sends via provider adapter" do
    laravel_request_fn = fn url, headers, body, timeout_ms ->
      send(self(), {:laravel_secret_fetch, url, headers, body, timeout_ms})
      {:ok, 200, ~s({"data":{"secret":"re_live_matrix_201","cache_ttl_ms":2000}})}
    end

    provider_http_client = fn endpoint, api_key, payload, timeout_ms ->
      send(self(), {:provider_send, endpoint, api_key, payload, timeout_ms})
      {:ok, 200, ~s({"id":"email_matrix_201"})}
    end

    request =
      base_request()
      |> Map.put("consent_granted", true)
      |> Map.put("user_id", 42)
      |> Map.put("run_id", "run_matrix_201")
      |> Map.put("tool_call_id", "tool_matrix_201")

    assert {:ok, outcome} =
             Kernel.execute_send(valid_manifest(), request,
               authorization_id: "auth_inc_201",
               adapter: ResendAdapter,
               from: "noreply@example.com",
               secret_client: LaravelSecretClient,
               secret_client_opts: laravel_secret_opts(laravel_request_fn, 20_000),
               http_client: provider_http_client
             )

    assert_receive {:laravel_secret_fetch, url, headers, body, timeout_ms}
    assert url == "http://laravel.test/api/internal/runtime/integrations/secrets/fetch"
    assert timeout_ms == 1_200
    assert headers["x-oa-internal-key-id"] == "runtime-internal-v1"
    assert is_binary(headers["x-oa-internal-signature"])
    assert {:ok, payload} = Jason.decode(body)
    assert payload["user_id"] == 42
    assert payload["integration_id"] == "resend.primary"
    assert payload["run_id"] == "run_matrix_201"
    assert payload["tool_call_id"] == "tool_matrix_201"

    assert_receive {:provider_send, endpoint, api_key, provider_payload, provider_timeout_ms}
    assert endpoint == "https://api.resend.com/emails"
    assert api_key == "re_live_matrix_201"
    assert provider_timeout_ms == 10_000
    assert provider_payload["from"] == "noreply@example.com"
    assert Enum.any?(provider_payload["tags"], &(&1["name"] == "tool_call_id"))

    assert outcome["state"] == "sent"
    assert outcome["reason_code"] == "policy_allowed.default"
    assert outcome["receipt"]["state"] == "sent"
    assert outcome["receipt"]["reason_code"] == "policy_allowed.default"
  end

  test "INC-COMMS-202 revoke flow denies send for next execution scope" do
    laravel_request_fn = fn _url, _headers, body, _timeout_ms ->
      assert {:ok, payload} = Jason.decode(body)

      case payload["tool_call_id"] do
        "tool_matrix_202_live" ->
          {:ok, 200, ~s({"data":{"secret":"re_live_matrix_202","cache_ttl_ms":2000}})}

        "tool_matrix_202_revoked" ->
          {:ok, 404, ~s({"error":{"code":"secret_not_found"}})}
      end
    end

    provider_http_client = fn _endpoint, api_key, _payload, _timeout_ms ->
      send(self(), {:provider_send, api_key})
      {:ok, 200, ~s({"id":"email_matrix_202"})}
    end

    active_request =
      base_request()
      |> Map.put("user_id", 42)
      |> Map.put("run_id", "run_matrix_202")
      |> Map.put("tool_call_id", "tool_matrix_202_live")

    assert {:ok, active_result} =
             ResendAdapter.send(active_request, %{},
               from: "noreply@example.com",
               secret_client: LaravelSecretClient,
               secret_client_opts: laravel_secret_opts(laravel_request_fn, 20_001),
               http_client: provider_http_client
             )

    assert active_result["state"] == "sent"
    assert_receive {:provider_send, "re_live_matrix_202"}

    revoked_request = Map.put(active_request, "tool_call_id", "tool_matrix_202_revoked")

    assert {:error, revoked_error} =
             ResendAdapter.send(revoked_request, %{},
               from: "noreply@example.com",
               secret_client: LaravelSecretClient,
               secret_client_opts: laravel_secret_opts(laravel_request_fn, 20_002),
               http_client: fn _endpoint, _api_key, _payload, _timeout_ms ->
                 flunk("provider call should not execute when scoped secret is revoked")
               end
             )

    assert revoked_error["reason_code"] == "policy_denied.explicit_deny"
    assert revoked_error["state"] == "failed"
    assert revoked_error["message"] == "missing_api_key"
  end

  test "INC-COMMS-203 replay parity matches blocked execution policy decision" do
    request = Map.put(base_request(), "consent_granted", true)

    opts = [
      authorization_id: "auth_inc_203",
      suppressed_recipients: ["user@example.com"]
    ]

    assert {:ok, execution_outcome} = Kernel.execute_send(valid_manifest(), request, opts)
    assert execution_outcome["state"] == "blocked"
    assert execution_outcome["decision"] == "denied"
    assert execution_outcome["reason_code"] == "policy_denied.suppressed_recipient"
    assert execution_outcome["receipt"]["reason_code"] == "policy_denied.suppressed_recipient"

    assert {:ok, replay_a} = Kernel.replay_decision(valid_manifest(), request, opts)
    assert {:ok, replay_b} = Kernel.replay_decision(valid_manifest(), request, opts)

    assert replay_a == replay_b
    assert replay_a["decision"] == execution_outcome["decision"]
    assert replay_a["reason_code"] == execution_outcome["reason_code"]
    assert replay_a["evaluation_hash"] == execution_outcome["policy"]["evaluation_hash"]
    assert String.length(replay_a["replay_hash"]) == 64
  end

  defp laravel_secret_opts(request_fn, now_ms) do
    [
      request_fn: request_fn,
      now_ms: now_ms,
      base_url: "http://laravel.test",
      secret_fetch_path: "/api/internal/runtime/integrations/secrets/fetch",
      shared_secret: "test-runtime-internal-shared-secret",
      key_id: "runtime-internal-v1",
      signature_ttl_seconds: 60,
      request_timeout_ms: 1_200,
      default_secret_cache_ttl_ms: 250
    ]
  end

  defp valid_manifest do
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

  defp base_request do
    %{
      "integration_id" => "resend.primary",
      "recipient" => "user@example.com",
      "template_id" => "welcome_email",
      "variables" => %{"first_name" => "Casey"}
    }
  end
end
