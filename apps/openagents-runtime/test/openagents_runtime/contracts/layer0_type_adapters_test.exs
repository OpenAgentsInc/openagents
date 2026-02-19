defmodule OpenAgentsRuntime.Contracts.Layer0TypeAdaptersTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Contracts.Layer0TypeAdapters
  alias OpenAgentsRuntime.DS.Receipts

  test "run_event/4 adapts runtime event payloads to proto-compatible run event maps" do
    assert {:ok, run_started} =
             Layer0TypeAdapters.run_event("run_1", 1, "run.started", %{"actor" => "assistant"})

    assert run_started["run_id"] == "run_1"
    assert run_started["run_started"]["actor"] == "assistant"

    assert {:ok, tool_result} =
             Layer0TypeAdapters.run_event("run_1", 2, "tool.result", %{
               "tool_call_id" => "tool_1",
               "tool_name" => "web.search",
               "state" => "succeeded",
               "reason_code" => "policy_allowed.default",
               "output" => %{"result" => "ok"}
             })

    assert tool_result["tool_result"]["reason_code"] == "REASON_CODE_POLICY_ALLOWED_DEFAULT"
  end

  test "predict_receipt/1 adapts deterministic DS receipts to proto-compatible map" do
    receipt =
      Receipts.build_predict(%{
        run_id: "run_predict_1",
        signature_id: "sig.alpha.v1",
        strategy_id: "direct.v1",
        compiled_id: "compiled.alpha",
        schema_hash: String.duplicate("a", 64),
        prompt_hash: String.duplicate("b", 64),
        program_hash: String.duplicate("c", 64),
        params_hash: String.duplicate("d", 64),
        output_hash: String.duplicate("e", 64),
        policy: %{
          "policy_id" => "ds.predict.v1",
          "authorization_id" => "auth_123",
          "authorization_mode" => "delegated_budget",
          "decision" => "allowed",
          "reason_code" => "policy_allowed.default",
          "reason_codes_version" => "runtime-policy-reasons.v1",
          "evaluation_hash" => String.duplicate("f", 64)
        },
        budget: %{"spent_sats" => 1},
        timing: %{"latency_ms" => 10},
        catalog_version: 1
      })

    assert {:ok, adapted} = Layer0TypeAdapters.predict_receipt(receipt)
    assert adapted["receipt_id"] =~ "pred_"
    assert adapted["policy"]["reason_code"] == "REASON_CODE_POLICY_ALLOWED_DEFAULT"
    assert adapted["policy"]["reason_code_text"] == "policy_allowed.default"
  end

  test "comms adapters validate intent and result payloads against proto contracts" do
    assert {:ok, intent} =
             Layer0TypeAdapters.comms_send_intent(
               %{"provider" => "resend"},
               %{
                 "org_id" => "org_1",
                 "user_id" => "42",
                 "channel" => "email",
                 "template_id" => "welcome",
                 "recipient" => "user@example.com",
                 "variables" => %{"name" => "Casey"}
               }
             )

    assert intent["provider"] == "resend"

    assert {:ok, result} =
             Layer0TypeAdapters.comms_send_result(%{
               "message_id" => "msg_123",
               "state" => "sent",
               "reason_code" => "policy_allowed.default",
               "provider_result" => %{"provider_status" => 202}
             })

    assert result["reason_code"] == "REASON_CODE_POLICY_ALLOWED_DEFAULT"
    assert result["reason_code_text"] == "policy_allowed.default"
  end

  test "adapters reject reason codes that cannot be represented by proto enum" do
    assert {:error, errors} =
             Layer0TypeAdapters.comms_send_result(%{
               "message_id" => "msg_123",
               "state" => "failed",
               "reason_code" => "unknown_reason.not_supported"
             })

    assert Enum.any?(errors, &String.contains?(&1, "unsupported reason code"))
  end
end
