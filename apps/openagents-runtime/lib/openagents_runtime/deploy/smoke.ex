defmodule OpenAgentsRuntime.Deploy.Smoke do
  @moduledoc """
  Post-deploy smoke checks for health, stream integrity, and tool execution.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Runs.RunFrame
  alias OpenAgentsRuntime.Runs.RunLease
  alias OpenAgentsRuntime.Runs.RunOwnership
  alias OpenAgentsRuntime.Tools.ToolRunner
  alias OpenAgentsRuntime.Tools.ToolTask
  alias OpenAgentsRuntime.Tools.ToolTasks

  @default_user_id 9_001

  @type option ::
          {:base_url, String.t()}
          | {:tail_ms, pos_integer()}
          | {:user_id, pos_integer()}
          | {:run_id, String.t()}
          | {:thread_id, String.t()}
          | {:http_get,
             (String.t(), [{String.t(), String.t()}] ->
                {:ok, pos_integer(), String.t() | binary()} | {:error, term()})}

  @spec run!([option()]) :: :ok | no_return()
  def run!(opts \\ []) when is_list(opts) do
    run_id = Keyword.get(opts, :run_id, "smoke_run_#{System.unique_integer([:positive])}")
    thread_id = Keyword.get(opts, :thread_id, "thread_#{run_id}")
    user_id = Keyword.get(opts, :user_id, @default_user_id)
    tail_ms = Keyword.get(opts, :tail_ms, 120)
    base_url = normalize_base_url(Keyword.get(opts, :base_url, default_base_url()))
    http_get_fun = Keyword.get(opts, :http_get, &http_get/2)

    try do
      with :ok <- health_check(base_url, http_get_fun),
           :ok <- seed_run(run_id, thread_id, user_id),
           :ok <- seed_stream_events(run_id),
           :ok <- stream_check(base_url, run_id, thread_id, user_id, tail_ms, http_get_fun),
           :ok <- tool_path_check(run_id) do
        :ok
      else
        {:error, reason} ->
          raise "runtime smoke check failed: #{inspect(reason)}"
      end
    after
      cleanup_run(run_id, thread_id)
    end
  end

  defp health_check(base_url, http_get_fun) do
    case http_get_fun.("#{base_url}/internal/v1/health", []) do
      {:ok, 200, body} ->
        if String.contains?(body, "ok") do
          :ok
        else
          {:error, {:health_unexpected_body, body}}
        end

      {:ok, status, body} ->
        {:error, {:health_status, status, body}}

      {:error, reason} ->
        {:error, {:health_http_error, reason}}
    end
  end

  defp seed_run(run_id, thread_id, user_id) do
    {:ok, _run} =
      Repo.insert(%Run{
        run_id: run_id,
        thread_id: thread_id,
        status: "running",
        owner_user_id: user_id,
        latest_seq: 0
      })

    {:ok, _ownership} =
      Repo.insert(%RunOwnership{
        run_id: run_id,
        thread_id: thread_id,
        user_id: user_id
      })

    :ok
  rescue
    error -> {:error, {:seed_run_failed, error}}
  end

  defp seed_stream_events(run_id) do
    with {:ok, _} <- RunEvents.append_event(run_id, "run.delta", %{"delta" => "smoke"}),
         {:ok, _} <-
           RunEvents.append_event(run_id, "run.finished", %{
             "status" => "succeeded",
             "reason_class" => "completed",
             "reason" => "smoke_test"
           }) do
      :ok
    else
      {:error, reason} -> {:error, {:seed_stream_failed, reason}}
    end
  end

  defp stream_check(base_url, run_id, thread_id, user_id, tail_ms, http_get_fun) do
    with {:ok, token} <- signed_internal_token(run_id, thread_id, user_id),
         {:ok, status, body} <-
           http_get_fun.(
             "#{base_url}/internal/v1/runs/#{run_id}/stream?thread_id=#{thread_id}&cursor=0&tail_ms=#{tail_ms}",
             [
               {"x-oa-runtime-signature", token},
               {"x-oa-user-id", Integer.to_string(user_id)}
             ]
           ) do
      cond do
        status != 200 ->
          {:error, {:stream_status, status, body}}

        not String.contains?(body, "[DONE]") ->
          {:error, {:stream_missing_done, body}}

        not String.contains?(body, "\"text-delta\"") ->
          {:error, {:stream_missing_delta, body}}

        true ->
          :ok
      end
    else
      {:error, reason} -> {:error, {:stream_check_failed, reason}}
    end
  end

  defp tool_path_check(run_id) do
    tool_call_id = "smoke_tool_#{System.unique_integer([:positive])}"

    with {:ok, %{"result" => "ok"}} <-
           ToolRunner.run(
             fn ->
               %{"result" => "ok"}
             end,
             run_id: run_id,
             tool_call_id: tool_call_id,
             tool_name: "smoke.tool",
             timeout_ms: 1_000
           ),
         %ToolTask{state: "succeeded"} <- ToolTasks.get_by_tool_call(run_id, tool_call_id),
         true <- tool_events_present?(run_id, tool_call_id) do
      :ok
    else
      {:error, reason} ->
        {:error, {:tool_runner_failed, reason}}

      nil ->
        {:error, :tool_task_missing}

      false ->
        {:error, :tool_events_missing}

      other ->
        {:error, {:tool_path_failed, other}}
    end
  end

  defp tool_events_present?(run_id, tool_call_id) do
    events = RunEvents.list_after(run_id, 0)

    has_call? =
      Enum.any?(events, fn event ->
        event.event_type == "tool.call" and event.payload["tool_call_id"] == tool_call_id
      end)

    has_result? =
      Enum.any?(events, fn event ->
        event.event_type == "tool.result" and event.payload["tool_call_id"] == tool_call_id and
          event.payload["status"] == "succeeded"
      end)

    has_call? and has_result?
  end

  defp cleanup_run(run_id, thread_id) do
    Repo.delete_all(from(event in RunEvent, where: event.run_id == ^run_id))
    Repo.delete_all(from(frame in RunFrame, where: frame.run_id == ^run_id))
    Repo.delete_all(from(task in ToolTask, where: task.run_id == ^run_id))
    Repo.delete_all(from(lease in RunLease, where: lease.run_id == ^run_id))
    Repo.delete_all(from(ownership in RunOwnership, where: ownership.run_id == ^run_id))

    Repo.delete_all(
      from(run in Run, where: run.run_id == ^run_id and run.thread_id == ^thread_id)
    )

    :ok
  end

  defp signed_internal_token(run_id, thread_id, user_id) do
    with secret when is_binary(secret) and byte_size(secret) > 0 <-
           Application.get_env(:openagents_runtime, :runtime_signature_secret) do
      now = System.system_time(:second)

      claims = %{
        "iat" => now,
        "exp" => now + 300,
        "nonce" => "smoke-#{System.unique_integer([:positive])}",
        "run_id" => run_id,
        "thread_id" => thread_id,
        "user_id" => user_id
      }

      payload_segment = claims |> Jason.encode!() |> Base.url_encode64(padding: false)
      signature = :crypto.mac(:hmac, :sha256, secret, payload_segment)
      signature_segment = Base.url_encode64(signature, padding: false)

      {:ok, "v1.#{payload_segment}.#{signature_segment}"}
    else
      _ -> {:error, :runtime_signature_secret_missing}
    end
  end

  defp default_base_url do
    namespace = System.get_env("POD_NAMESPACE") || "default"
    "http://openagents-runtime.#{namespace}.svc.cluster.local"
  end

  defp normalize_base_url(base_url) do
    base_url
    |> String.trim()
    |> String.trim_trailing("/")
  end

  defp http_get(url, headers) do
    _ = :inets.start()
    _ = :ssl.start()

    normalized_headers =
      Enum.map(headers, fn {key, value} ->
        {String.to_charlist(key), String.to_charlist(value)}
      end)

    request = {String.to_charlist(url), normalized_headers}
    http_options = [timeout: 5_000, connect_timeout: 2_000]
    request_options = [body_format: :binary]

    case :httpc.request(:get, request, http_options, request_options) do
      {:ok, {{_version, status_code, _reason_phrase}, _response_headers, body}} ->
        {:ok, status_code, body}

      {:error, reason} ->
        {:error, reason}
    end
  end
end
