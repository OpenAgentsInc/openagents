defmodule OpenAgentsRuntime.Security.SanitizerTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Security.Sanitizer

  test "redacts sensitive keys and PII" do
    input = %{
      "authorization" => "Bearer abc.def.ghi",
      "api_key" => "sk-live-secret",
      "email" => "user@example.com",
      "phone" => "+1 (555) 123-4567",
      "nested" => %{
        "refresh_token" => "refresh-secret",
        "safe" => "ok"
      }
    }

    sanitized = Sanitizer.sanitize(input)

    assert sanitized["authorization"] == "[REDACTED]"
    assert sanitized["api_key"] == "[REDACTED]"
    assert sanitized["email"] == "[REDACTED_EMAIL]"
    assert sanitized["phone"] == "[REDACTED_PHONE]"
    assert sanitized["nested"]["refresh_token"] == "[REDACTED]"
    assert sanitized["nested"]["safe"] == "ok"
  end

  test "redacts secret-like substrings in freeform strings" do
    input =
      "token=sk-live-123 and Bearer aaa.bbb.ccc and email foo@bar.com phone +15551234567"

    output = Sanitizer.sanitize(input)

    refute String.contains?(output, "sk-live-123")
    refute String.contains?(output, "aaa.bbb.ccc")
    refute String.contains?(output, "foo@bar.com")
    refute String.contains?(output, "+15551234567")
    assert String.contains?(output, "[REDACTED]")
  end

  test "preserve_keys keeps non-secret operational keys intact" do
    input = %{
      run_id: "run_123",
      authorization_mode: "delegated_budget",
      token_count: 42
    }

    sanitized =
      Sanitizer.sanitize(input, preserve_keys: [:run_id, :authorization_mode, :token_count])

    assert sanitized.run_id == "run_123"
    assert sanitized.authorization_mode == "delegated_budget"
    assert sanitized.token_count == 42
  end
end
