defmodule OpenAgentsRuntime.Integrations.LaravelSecretClientTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Integrations.LaravelSecretClient

  setup do
    :ok = LaravelSecretClient.clear_cache()
    :ok
  end

  test "fetch_secret/3 signs request and reuses short-lived scoped cache" do
    request_fn = fn url, headers, body, timeout_ms ->
      send(self(), {:secret_request, url, headers, body, timeout_ms})
      {:ok, 200, ~s({"data":{"secret":"re_live_from_laravel","cache_ttl_ms":2000}})}
    end

    opts =
      base_opts(
        request_fn: request_fn,
        now_ms: 10_000,
        request_timeout_ms: 1_250,
        default_secret_cache_ttl_ms: 250
      )

    scope = base_scope()

    assert {:ok, "re_live_from_laravel"} = LaravelSecretClient.fetch_secret("resend", scope, opts)

    assert_receive {:secret_request, url, headers, body, timeout_ms}
    assert url == "http://laravel.test/api/internal/runtime/integrations/secrets/fetch"
    assert timeout_ms == 1_250
    assert headers["x-oa-internal-key-id"] == "runtime-internal-v1"
    assert headers["x-oa-internal-timestamp"] == "10"

    body_hash = :crypto.hash(:sha256, body) |> Base.encode16(case: :lower)
    assert headers["x-oa-internal-body-sha256"] == body_hash

    expected_signature =
      :crypto.mac(
        :hmac,
        :sha256,
        "test-runtime-internal-shared-secret",
        "10\n#{headers["x-oa-internal-nonce"]}\n#{body_hash}"
      )
      |> Base.encode16(case: :lower)

    assert headers["x-oa-internal-signature"] == expected_signature

    assert {:ok, payload} = Jason.decode(body)
    assert payload["user_id"] == 42
    assert payload["provider"] == "resend"
    assert payload["integration_id"] == "resend.primary"
    assert payload["run_id"] == "run_1"
    assert payload["tool_call_id"] == "tool_1"

    cache_only_opts =
      base_opts(
        request_fn: fn _url, _headers, _body, _timeout_ms ->
          flunk("request_fn should not be called when scoped cache hit occurs")
        end,
        now_ms: 11_000
      )

    assert {:ok, "re_live_from_laravel"} =
             LaravelSecretClient.fetch_secret("resend", scope, cache_only_opts)
  end

  test "fetch_secret/3 returns secret_not_found on revoked integration for next execution scope" do
    request_fn = fn _url, _headers, body, _timeout_ms ->
      assert {:ok, payload} = Jason.decode(body)

      case payload["tool_call_id"] do
        "tool_1" ->
          {:ok, 200, ~s({"data":{"secret":"re_live_from_laravel","cache_ttl_ms":2000}})}

        "tool_2" ->
          {:ok, 404, ~s({"error":{"code":"secret_not_found"}})}
      end
    end

    assert {:ok, "re_live_from_laravel"} =
             LaravelSecretClient.fetch_secret(
               "resend",
               base_scope(%{"tool_call_id" => "tool_1"}),
               base_opts(request_fn: request_fn, now_ms: 20_000)
             )

    assert {:error, :secret_not_found} =
             LaravelSecretClient.fetch_secret(
               "resend",
               base_scope(%{"tool_call_id" => "tool_2"}),
               base_opts(request_fn: request_fn, now_ms: 20_001)
             )
  end

  test "fetch_secret/3 enforces scope validation and auth failures" do
    assert {:error, :invalid_scope} =
             LaravelSecretClient.fetch_secret(
               "resend",
               %{"user_id" => 42},
               base_opts(
                 request_fn: fn _url, _headers, _body, _timeout_ms ->
                   flunk("request should not be executed for invalid scope")
                 end
               )
             )

    unauthorized_fn = fn _url, _headers, _body, _timeout_ms ->
      {:ok, 401, ~s({"error":{"code":"invalid_signature"}})}
    end

    assert {:error, :unauthorized} =
             LaravelSecretClient.fetch_secret(
               "resend",
               base_scope(),
               base_opts(request_fn: unauthorized_fn)
             )
  end

  defp base_scope(overrides \\ %{}) do
    Map.merge(
      %{
        "user_id" => 42,
        "integration_id" => "resend.primary",
        "run_id" => "run_1",
        "tool_call_id" => "tool_1"
      },
      overrides
    )
  end

  defp base_opts(overrides) do
    [
      base_url: "http://laravel.test",
      secret_fetch_path: "/api/internal/runtime/integrations/secrets/fetch",
      shared_secret: "test-runtime-internal-shared-secret",
      key_id: "runtime-internal-v1",
      signature_ttl_seconds: 60,
      request_timeout_ms: 1_000,
      default_secret_cache_ttl_ms: 100
    ]
    |> Keyword.merge(overrides)
  end
end
