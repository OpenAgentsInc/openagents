defmodule OpenAgentsRuntime.Tools.Network.GuardedHTTPTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Tools.Network.GuardedHTTP

  test "blocks private address targets before transport and emits blocked audit event" do
    handler_id = "guarded-http-blocked-#{System.unique_integer([:positive])}"
    parent = self()

    :ok =
      :telemetry.attach(
        handler_id,
        [:openagents_runtime, :tools, :network, :blocked],
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:blocked_event, measurements, metadata})
        end,
        parent
      )

    on_exit(fn -> :telemetry.detach(handler_id) end)

    transport = fn _method, _url, _headers, _body, _opts ->
      send(parent, :transport_called)
      {:ok, 200, [], ~s({"ok":true})}
    end

    assert {:error, {:blocked, "ssrf_block.private_address", details}} =
             GuardedHTTP.request(:post, "http://127.0.0.1:4000/send", [], "{}",
               transport: transport
             )

    assert details["host"] == "127.0.0.1"
    refute_receive :transport_called
    assert_receive {:blocked_event, %{count: 1}, metadata}
    assert metadata.reason_code == "ssrf_block.private_address"
  end

  test "blocks metadata endpoint hostnames deterministically" do
    assert {:error, {:blocked, "ssrf_block.metadata_endpoint", details}} =
             GuardedHTTP.request(
               :get,
               "http://metadata.google.internal/computeMetadata/v1",
               [],
               ""
             )

    assert details["host"] == "metadata.google.internal"
  end

  test "blocks hosts outside allowlist" do
    dns_resolver = fn "example.com" -> {:ok, [{8, 8, 8, 8}]} end

    assert {:error, {:blocked, "ssrf_block.host_not_allowed", details}} =
             GuardedHTTP.request(
               :get,
               "https://example.com/data",
               [],
               "",
               allowed_hosts: ["api.resend.com"],
               dns_resolver: dns_resolver
             )

    assert details["host"] == "example.com"
  end

  test "follows redirects up to configured bound" do
    transport = fn _method, url, _headers, _body, _opts ->
      case url do
        "https://api.resend.com/start" ->
          {:ok, 302, [{"location", "/next"}], ""}

        "https://api.resend.com/next" ->
          {:ok, 200, [], ~s({"id":"email_1"})}
      end
    end

    dns_resolver = fn "api.resend.com" -> {:ok, [{1, 1, 1, 1}]} end

    assert {:ok, 200, body} =
             GuardedHTTP.request(
               :post,
               "https://api.resend.com/start",
               [],
               "{}",
               max_redirects: 2,
               allowed_hosts: ["api.resend.com"],
               dns_resolver: dns_resolver,
               transport: transport
             )

    assert body == ~s({"id":"email_1"})
  end

  test "blocks when redirect chain exceeds bound" do
    transport = fn _method, _url, _headers, _body, _opts ->
      {:ok, 302, [{"location", "/again"}], ""}
    end

    dns_resolver = fn "api.resend.com" -> {:ok, [{1, 1, 1, 1}]} end

    assert {:error, {:blocked, "ssrf_block.redirect_limit_exceeded", _details}} =
             GuardedHTTP.request(
               :get,
               "https://api.resend.com/start",
               [],
               "",
               max_redirects: 1,
               allowed_hosts: ["api.resend.com"],
               dns_resolver: dns_resolver,
               transport: transport
             )
  end

  test "blocks DNS pin mismatch across redirect chain" do
    parent = self()
    Process.put(:dns_pin_count, 0)

    dns_resolver = fn "api.resend.com" ->
      count = Process.get(:dns_pin_count, 0)
      Process.put(:dns_pin_count, count + 1)

      case count do
        0 -> {:ok, [{1, 1, 1, 1}]}
        _ -> {:ok, [{2, 2, 2, 2}]}
      end
    end

    transport = fn _method, url, _headers, _body, _opts ->
      send(parent, {:transport_called, url})

      case url do
        "https://api.resend.com/start" ->
          {:ok, 302, [{"location", "https://api.resend.com/next"}], ""}

        "https://api.resend.com/next" ->
          {:ok, 200, [], ~s({"id":"unexpected"})}
      end
    end

    assert {:error, {:blocked, "ssrf_block.dns_pin_mismatch", _details}} =
             GuardedHTTP.request(
               :get,
               "https://api.resend.com/start",
               [],
               "",
               max_redirects: 2,
               allowed_hosts: ["api.resend.com"],
               dns_resolver: dns_resolver,
               transport: transport
             )

    assert_receive {:transport_called, "https://api.resend.com/start"}
    refute_receive {:transport_called, "https://api.resend.com/next"}
  end

  test "feature flag can disable network guard for controlled rollout" do
    transport = fn _method, _url, _headers, _body, _opts ->
      {:ok, 200, [], ~s({"ok":true})}
    end

    assert {:ok, 200, ~s({"ok":true})} =
             GuardedHTTP.request(
               :post,
               "http://127.0.0.1:4000/send",
               [],
               "{}",
               guard_enabled: false,
               transport: transport
             )
  end
end
