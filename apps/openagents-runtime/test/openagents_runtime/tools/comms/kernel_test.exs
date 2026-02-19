defmodule OpenAgentsRuntime.Tools.Comms.KernelTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Comms.Kernel

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
