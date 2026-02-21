defmodule OpenAgentsRuntime.Deploy.SmokeTest do
  use OpenAgentsRuntime.DataCase, async: false

  import Ecto.Query

  alias OpenAgentsRuntime.Deploy.Smoke
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent

  test "run!/1 validates health, stream, and tool path with injected HTTP client" do
    run_id = "smoke_test_#{System.unique_integer([:positive])}"
    thread_id = "thread_#{run_id}"
    user_id = 9_101
    {:ok, calls} = Agent.start_link(fn -> [] end)

    http_get = fn url, headers ->
      Agent.update(calls, fn acc -> [{url, headers} | acc] end)

      cond do
        String.ends_with?(url, "/internal/v1/health") ->
          {:ok, 200, ~s({"status":"ok"})}

        String.contains?(url, "/stream?") ->
          header_map = Map.new(headers)
          assert is_binary(header_map["x-oa-runtime-signature"])
          assert header_map["x-oa-user-id"] == Integer.to_string(user_id)

          {:ok, 200,
           "id: 1\ndata: {\"type\":\"text-delta\",\"delta\":\"smoke\"}\n\nid: 2\ndata: [DONE]\n\n"}

        true ->
          {:error, {:unexpected_url, url}}
      end
    end

    assert :ok =
             Smoke.run!(
               base_url: "http://runtime.local",
               run_id: run_id,
               thread_id: thread_id,
               user_id: user_id,
               http_get: http_get
             )

    urls = calls |> Agent.get(& &1) |> Enum.map(&elem(&1, 0))
    assert Enum.any?(urls, &String.ends_with?(&1, "/internal/v1/health"))
    assert Enum.any?(urls, &String.contains?(&1, "/stream?"))

    assert Repo.get(Run, run_id) == nil

    event_count =
      from(event in RunEvent, where: event.run_id == ^run_id, select: count())
      |> Repo.one()

    assert event_count == 0
  end

  test "run!/1 raises when stream output fails smoke invariants" do
    run_id = "smoke_fail_#{System.unique_integer([:positive])}"
    thread_id = "thread_#{run_id}"

    http_get = fn url, _headers ->
      cond do
        String.ends_with?(url, "/internal/v1/health") ->
          {:ok, 200, ~s({"status":"ok"})}

        String.contains?(url, "/stream?") ->
          {:ok, 200, "id: 1\ndata: {\"type\":\"text-delta\"}\n\n"}

        true ->
          {:error, {:unexpected_url, url}}
      end
    end

    assert_raise RuntimeError, ~r/stream_missing_done/, fn ->
      Smoke.run!(
        base_url: "http://runtime.local",
        run_id: run_id,
        thread_id: thread_id,
        user_id: 9_102,
        http_get: http_get
      )
    end

    assert Repo.get(Run, run_id) == nil
  end
end
