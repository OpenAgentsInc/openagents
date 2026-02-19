defmodule OpenAgentsRuntime.Tools.Comms.KernelTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Comms.Kernel
  alias OpenAgentsRuntime.Tools.ProviderCircuitBreaker

  defmodule SuccessAdapter do
    @behaviour OpenAgentsRuntime.Tools.Comms.ProviderAdapter

    @impl true
    def send(_request, _manifest, _opts) do
      {:ok, %{"message_id" => "msg_test_001", "state" => "sent"}}
    end
  end

  defmodule FailingAdapter do
    @behaviour OpenAgentsRuntime.Tools.Comms.ProviderAdapter

    @impl true
    def send(_request, _manifest, _opts) do
      {:error, %{reason: "provider unavailable"}}
    end
  end

  defmodule FallbackAdapter do
    @behaviour OpenAgentsRuntime.Tools.Comms.ProviderAdapter

    @impl true
    def send(_request, _manifest, _opts) do
      {:ok, %{"message_id" => "msg_fallback_001", "state" => "sent"}}
    end
  end

  defmodule LeakyAdapter do
    @behaviour OpenAgentsRuntime.Tools.Comms.ProviderAdapter

    @impl true
    def send(_request, _manifest, _opts) do
      {:ok,
       %{
         "message_id" => "msg_redacted_001",
         "state" => "sent",
         "authorization" => "Bearer abc.def.ghi",
         "api_key" => "sk-live-secret-token",
         "email" => "provider@example.com",
         "nested" => %{
           "client_secret" => "nested-secret"
         }
       }}
    end
  end

  setup do
    ProviderCircuitBreaker.reset(:all)
    :ok
  end

  test "execute_send/3 blocks when consent is required but not granted" do
    assert {:ok, outcome} =
             Kernel.execute_send(valid_manifest(), base_request(),
               authorization_id: "auth_123",
               adapter: SuccessAdapter
             )

    assert outcome["state"] == "blocked"
    assert outcome["decision"] == "denied"
    assert outcome["reason_code"] == "policy_denied.consent_required"
    assert outcome["receipt"]["reason_code"] == "policy_denied.consent_required"
  end

  test "execute_send/3 blocks suppressed recipients when suppression policy enforces" do
    request = Map.put(base_request(), "consent_granted", true)

    assert {:ok, outcome} =
             Kernel.execute_send(valid_manifest(), request,
               authorization_id: "auth_123",
               suppressed_recipients: ["user@example.com"],
               adapter: SuccessAdapter
             )

    assert outcome["state"] == "blocked"
    assert outcome["reason_code"] == "policy_denied.suppressed_recipient"
  end

  test "execute_send/3 sends when policy gates pass" do
    request = Map.put(base_request(), "consent_granted", true)

    assert {:ok, outcome} =
             Kernel.execute_send(valid_manifest(), request,
               authorization_id: "auth_123",
               adapter: SuccessAdapter
             )

    assert outcome["state"] == "sent"
    assert outcome["decision"] == "allowed"
    assert outcome["reason_code"] == "policy_allowed.default"
    assert outcome["message_id"] == "msg_test_001"
    assert outcome["receipt"]["state"] == "sent"
  end

  test "execute_send/3 marks provider adapter failures as failed receipts" do
    request = Map.put(base_request(), "consent_granted", true)

    assert {:ok, outcome} =
             Kernel.execute_send(valid_manifest(), request,
               authorization_id: "auth_123",
               adapter: FailingAdapter
             )

    assert outcome["state"] == "failed"
    assert outcome["decision"] == "denied"
    assert outcome["reason_code"] == "comms_failed.provider_error"
    assert outcome["receipt"]["state"] == "failed"
  end

  test "execute_send/3 returns circuit-open reason after breaker trips" do
    request = Map.put(base_request(), "consent_granted", true)

    assert {:ok, first_outcome} =
             Kernel.execute_send(valid_manifest(), request,
               authorization_id: "auth_123",
               adapter: FailingAdapter,
               provider_failure_threshold: 1,
               provider_reset_timeout_ms: 500
             )

    assert first_outcome["reason_code"] == "comms_failed.provider_error"

    assert {:ok, second_outcome} =
             Kernel.execute_send(valid_manifest(), request,
               authorization_id: "auth_123",
               adapter: FailingAdapter,
               provider_failure_threshold: 1,
               provider_reset_timeout_ms: 500
             )

    assert second_outcome["state"] == "failed"
    assert second_outcome["reason_code"] == "comms_failed.provider_circuit_open"
    assert second_outcome["provider_result"]["failure_reason"] == "provider_circuit_open"
  end

  test "execute_send/3 uses fallback provider when enabled and primary breaker is open" do
    request = Map.put(base_request(), "consent_granted", true)

    assert {:ok, _first_outcome} =
             Kernel.execute_send(valid_manifest(), request,
               authorization_id: "auth_123",
               adapter: FailingAdapter,
               provider_failure_threshold: 1,
               provider_reset_timeout_ms: 500
             )

    assert {:ok, outcome} =
             Kernel.execute_send(valid_manifest(), request,
               authorization_id: "auth_123",
               adapter: FailingAdapter,
               provider_failure_threshold: 1,
               provider_reset_timeout_ms: 500,
               allow_provider_fallback: true,
               fallback_adapter: FallbackAdapter,
               fallback_provider: "resend.backup"
             )

    assert outcome["state"] == "sent"
    assert outcome["decision"] == "allowed"
    assert outcome["fallback"]["used"] == true
    assert outcome["fallback"]["provider"] == "resend.backup"
    assert outcome["fallback"]["primary_provider"] == "resend"
    assert outcome["receipt"]["provider"] == "resend.backup"
    assert outcome["reason_code"] == "policy_allowed.default"
  end

  test "execute_send/3 redacts secret material from provider and receipt output surfaces" do
    request =
      base_request()
      |> Map.put("consent_granted", true)
      |> Map.put("variables", %{
        "api_key" => "sk-live-input-token",
        "email" => "request@example.com",
        "safe" => "ok"
      })

    assert {:ok, outcome} =
             Kernel.execute_send(valid_manifest(), request,
               authorization_id: "auth_123",
               adapter: LeakyAdapter
             )

    assert outcome["state"] == "sent"
    assert outcome["reason_code"] == "policy_allowed.default"
    assert outcome["provider_result"]["authorization"] == "[REDACTED]"
    assert outcome["provider_result"]["api_key"] == "[REDACTED]"
    assert outcome["provider_result"]["email"] == "[REDACTED_EMAIL]"
    assert outcome["provider_result"]["nested"]["client_secret"] == "[REDACTED]"
    assert outcome["receipt"]["recipient"] == "[REDACTED_EMAIL]"

    encoded = inspect(outcome)
    refute String.contains?(encoded, "sk-live-secret-token")
    refute String.contains?(encoded, "nested-secret")
    refute String.contains?(encoded, "provider@example.com")
  end

  test "replay_decision/3 is deterministic and reason-coded" do
    request = Map.put(base_request(), "consent_granted", true)

    opts = [
      authorization_id: "auth_123",
      policy_context: %{loop_detected_reason: "loop_detected.no_progress"}
    ]

    assert {:ok, replay_a} = Kernel.replay_decision(valid_manifest(), request, opts)
    assert {:ok, replay_b} = Kernel.replay_decision(valid_manifest(), request, opts)

    assert replay_a == replay_b
    assert replay_a["decision"] == "denied"
    assert replay_a["reason_code"] == "loop_detected.no_progress"
    assert String.length(replay_a["replay_hash"]) == 64
    assert String.length(replay_a["evaluation_hash"]) == 64
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
