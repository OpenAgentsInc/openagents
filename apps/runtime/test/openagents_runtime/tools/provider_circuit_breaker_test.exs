defmodule OpenAgentsRuntime.Tools.ProviderCircuitBreakerTest do
  use ExUnit.Case, async: false

  alias OpenAgentsRuntime.Tools.ProviderCircuitBreaker

  setup do
    ProviderCircuitBreaker.reset(:all)
    :ok
  end

  test "opens breaker after threshold failures and short-circuits subsequent calls" do
    provider = "resend-primary"

    assert {:error, :provider_down} =
             ProviderCircuitBreaker.call(
               provider,
               fn -> {:error, :provider_down} end,
               failure_threshold: 1,
               reset_timeout_ms: 50
             )

    assert ProviderCircuitBreaker.current_state(provider).state == :open

    assert {:error, :circuit_open} =
             ProviderCircuitBreaker.call(
               provider,
               fn -> {:ok, %{"state" => "sent"}} end,
               failure_threshold: 1,
               reset_timeout_ms: 50
             )
  end

  test "half-open trial closes breaker on successful call" do
    provider = "resend-recovery"

    assert {:error, :provider_down} =
             ProviderCircuitBreaker.call(
               provider,
               fn -> {:error, :provider_down} end,
               failure_threshold: 1,
               reset_timeout_ms: 5
             )

    assert ProviderCircuitBreaker.current_state(provider).state == :open
    Process.sleep(10)

    assert {:ok, %{"state" => "sent"}} =
             ProviderCircuitBreaker.call(
               provider,
               fn -> {:ok, %{"state" => "sent"}} end,
               failure_threshold: 1,
               reset_timeout_ms: 5
             )

    assert ProviderCircuitBreaker.current_state(provider).state == :closed
    assert ProviderCircuitBreaker.current_state(provider).failure_count == 0
  end
end
