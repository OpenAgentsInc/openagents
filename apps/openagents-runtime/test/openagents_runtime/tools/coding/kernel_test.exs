defmodule OpenAgentsRuntime.Tools.Coding.KernelTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Coding.Kernel
  alias OpenAgentsRuntime.Tools.ProviderCircuitBreaker

  defmodule SuccessAdapter do
    @behaviour OpenAgentsRuntime.Tools.Coding.ProviderAdapter

    @impl true
    def execute(_request, _manifest, _opts) do
      {:ok, %{"state" => "succeeded", "data" => %{"id" => 123}}}
    end
  end

  defmodule FailingAdapter do
    @behaviour OpenAgentsRuntime.Tools.Coding.ProviderAdapter

    @impl true
    def execute(_request, _manifest, _opts) do
      {:error, %{"message" => "provider unavailable"}}
    end
  end

  setup do
    ProviderCircuitBreaker.reset(:all)
    :ok
  end

  test "execute_operation/3 blocks when write operation is not approved under enforce mode" do
    request =
      base_request()
      |> Map.put("operation", "add_issue_comment")
      |> Map.put("issue_number", 42)
      |> Map.put("body", "ship it")

    assert {:ok, outcome} =
             Kernel.execute_operation(valid_manifest(), request,
               authorization_id: "auth_123",
               authorization_mode: "delegated_budget",
               adapter: SuccessAdapter
             )

    assert outcome["state"] == "blocked"
    assert outcome["decision"] == "denied"
    assert outcome["reason_code"] == "policy_denied.explicit_deny"
    assert outcome["receipt"]["reason_code"] == "policy_denied.explicit_deny"
  end

  test "execute_operation/3 returns machine-readable manifest validation errors for invalid manifests" do
    invalid_manifest = Map.delete(valid_manifest(), "manifest_version")

    assert {:error, {:invalid_manifest, errors}} =
             Kernel.execute_operation(invalid_manifest, base_request(),
               authorization_id: "auth_123",
               authorization_mode: "delegated_budget",
               adapter: SuccessAdapter
             )

    assert is_list(errors)
    assert Enum.all?(errors, &(&1["reason_code"] == "manifest_validation.invalid_schema"))
    assert Enum.any?(errors, &(&1["path"] == "manifest_version"))
  end

  test "execute_operation/3 succeeds for read operations when policy gates pass" do
    request =
      base_request()
      |> Map.put("operation", "get_issue")
      |> Map.put("issue_number", 42)

    assert {:ok, outcome} =
             Kernel.execute_operation(valid_manifest(), request,
               authorization_id: "auth_123",
               authorization_mode: "delegated_budget",
               adapter: SuccessAdapter
             )

    assert outcome["state"] == "succeeded"
    assert outcome["decision"] == "allowed"
    assert outcome["reason_code"] == "policy_allowed.default"
    assert outcome["provider_result"]["data"]["id"] == 123
  end

  test "execute_operation/3 marks provider adapter failures as failed receipts" do
    request =
      base_request()
      |> Map.put("operation", "get_issue")
      |> Map.put("issue_number", 42)

    assert {:ok, outcome} =
             Kernel.execute_operation(valid_manifest(), request,
               authorization_id: "auth_123",
               authorization_mode: "delegated_budget",
               adapter: FailingAdapter
             )

    assert outcome["state"] == "failed"
    assert outcome["decision"] == "denied"
    assert outcome["reason_code"] == "coding_failed.provider_error"
  end

  test "execute_operation/3 returns circuit-open reason after breaker trips" do
    request =
      base_request()
      |> Map.put("operation", "get_issue")
      |> Map.put("issue_number", 42)

    assert {:ok, first_outcome} =
             Kernel.execute_operation(valid_manifest(), request,
               authorization_id: "auth_123",
               authorization_mode: "delegated_budget",
               adapter: FailingAdapter,
               provider_failure_threshold: 1,
               provider_reset_timeout_ms: 500
             )

    assert first_outcome["reason_code"] == "coding_failed.provider_error"

    assert {:ok, second_outcome} =
             Kernel.execute_operation(valid_manifest(), request,
               authorization_id: "auth_123",
               authorization_mode: "delegated_budget",
               adapter: FailingAdapter,
               provider_failure_threshold: 1,
               provider_reset_timeout_ms: 500
             )

    assert second_outcome["state"] == "failed"
    assert second_outcome["reason_code"] == "coding_failed.provider_circuit_open"
  end

  test "replay_decision/3 is deterministic and reason-coded" do
    request =
      base_request()
      |> Map.put("operation", "get_issue")
      |> Map.put("issue_number", 42)

    opts = [
      authorization_id: "auth_123",
      authorization_mode: "delegated_budget"
    ]

    assert {:ok, replay_a} = Kernel.replay_decision(valid_manifest(), request, opts)
    assert {:ok, replay_b} = Kernel.replay_decision(valid_manifest(), request, opts)

    assert replay_a == replay_b
    assert replay_a["decision"] == "allowed"
    assert replay_a["reason_code"] == "policy_allowed.default"
    assert String.length(replay_a["replay_hash"]) == 64
    assert String.length(replay_a["evaluation_hash"]) == 64
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

  defp base_request do
    %{
      "integration_id" => "github.primary",
      "repository" => "OpenAgentsInc/openagents"
    }
  end
end
