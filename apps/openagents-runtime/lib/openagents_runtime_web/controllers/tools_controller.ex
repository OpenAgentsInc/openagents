defmodule OpenAgentsRuntimeWeb.ToolsController do
  use OpenAgentsRuntimeWeb, :controller

  alias OpenAgentsRuntime.Telemetry.Tracing
  alias OpenAgentsRuntime.Tools.Coding.Kernel, as: CodingKernel
  alias OpenAgentsRuntime.Tools.Coding.Providers.GitHubAdapter

  @execute_mode "execute"
  @replay_mode "replay"
  @supported_modes MapSet.new([@execute_mode, @replay_mode])

  @spec execute(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def execute(conn, params) when is_map(params) do
    Tracing.with_phase_span(:tool, %{tool_pack: params["tool_pack"], mode: params["mode"]}, fn ->
      with {:ok, payload} <- validate_payload(params),
           {:ok, payload} <- enforce_user_context(conn, payload),
           {:ok, result} <- dispatch(payload) do
        json(conn, %{"data" => result})
      else
        {:error, {:bad_request, errors}} ->
          validation_error(conn, 400, errors)

        {:error, {:invalid_manifest, errors}} ->
          validation_error(conn, 422, errors)

        {:error, {:invalid_request, errors}} ->
          validation_error(conn, 422, errors)

        {:error, :forbidden} ->
          error(conn, 403, "forbidden", "request context does not match authenticated principal")
      end
    end)
  end

  defp dispatch(%{"tool_pack" => "coding.v1", "mode" => @execute_mode} = payload) do
    CodingKernel.execute_operation(
      payload["manifest"],
      payload["request"],
      build_coding_opts(payload)
    )
  end

  defp dispatch(%{"tool_pack" => "coding.v1", "mode" => @replay_mode} = payload) do
    CodingKernel.replay_decision(
      payload["manifest"],
      payload["request"],
      build_coding_opts(payload)
    )
  end

  defp dispatch(%{"tool_pack" => tool_pack}) do
    {:error, {:bad_request, ["tool_pack is not supported: #{tool_pack}"]}}
  end

  defp validate_payload(params) do
    tool_pack = normalize_optional_string(params["tool_pack"])
    mode = normalize_optional_string(params["mode"]) || @execute_mode
    manifest = params["manifest"]
    request = params["request"]
    policy = params["policy"] || %{}

    errors =
      []
      |> maybe_add_error(not is_binary(tool_pack), "tool_pack is required")
      |> maybe_add_error(
        not MapSet.member?(@supported_modes, mode),
        "mode must be execute or replay"
      )
      |> maybe_add_error(not is_map(manifest), "manifest must be an object")
      |> maybe_add_error(not is_map(request), "request must be an object")
      |> maybe_add_error(not is_map(policy), "policy must be an object when provided")
      |> Enum.reverse()

    if errors == [] do
      request =
        request
        |> normalize_map()
        |> maybe_put_if_absent("run_id", normalize_optional_string(params["run_id"]))
        |> maybe_put_if_absent("thread_id", normalize_optional_string(params["thread_id"]))
        |> maybe_put_if_absent("user_id", normalize_optional_integer(params["user_id"]))

      {:ok,
       %{
         "tool_pack" => tool_pack,
         "mode" => mode,
         "manifest" => normalize_map(manifest),
         "request" => request,
         "policy" => normalize_map(policy),
         "run_id" => normalize_optional_string(params["run_id"]),
         "thread_id" => normalize_optional_string(params["thread_id"]),
         "user_id" => normalize_optional_integer(params["user_id"])
       }}
    else
      {:error, {:bad_request, errors}}
    end
  end

  defp enforce_user_context(conn, payload) do
    case parse_user_id_header(conn) do
      nil ->
        {:error, {:bad_request, ["x-oa-user-id header is required"]}}

      header_user_id ->
        payload_user_id =
          normalize_optional_integer(
            payload["user_id"] || get_in(payload, ["request", "user_id"])
          )

        cond do
          is_nil(payload_user_id) ->
            {:ok, put_user_context(payload, header_user_id)}

          payload_user_id == header_user_id ->
            {:ok, put_user_context(payload, header_user_id)}

          true ->
            {:error, :forbidden}
        end
    end
  end

  defp put_user_context(payload, user_id) do
    payload
    |> Map.put("user_id", user_id)
    |> update_in(["request"], fn request ->
      request
      |> normalize_map()
      |> Map.put("user_id", user_id)
    end)
  end

  defp build_coding_opts(payload) do
    policy = payload["policy"] || %{}

    [
      authorization_id: normalize_optional_string(policy["authorization_id"]),
      authorization_mode: normalize_optional_string(policy["authorization_mode"]),
      budget: normalize_map(policy["budget"] || %{}),
      write_approved: policy["write_approved"] == true,
      policy: normalize_map(policy["policy"] || %{}),
      adapter:
        Application.get_env(
          :openagents_runtime,
          :runtime_tools_coding_adapter,
          GitHubAdapter
        )
    ]
  end

  defp parse_user_id_header(conn) do
    case conn |> get_req_header("x-oa-user-id") |> List.first() do
      value when is_binary(value) ->
        case Integer.parse(value) do
          {parsed, ""} when parsed > 0 -> parsed
          _ -> nil
        end

      _ ->
        nil
    end
  end

  defp normalize_map(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp normalize_map(_), do: %{}

  defp normalize_optional_string(nil), do: nil

  defp normalize_optional_string(value) when is_binary(value) do
    value
    |> String.trim()
    |> case do
      "" -> nil
      trimmed -> trimmed
    end
  end

  defp normalize_optional_string(value) when is_atom(value),
    do: value |> Atom.to_string() |> normalize_optional_string()

  defp normalize_optional_string(value) when is_integer(value),
    do: Integer.to_string(value)

  defp normalize_optional_string(_), do: nil

  defp normalize_optional_integer(value) when is_integer(value) and value > 0, do: value

  defp normalize_optional_integer(value) when is_binary(value) do
    case Integer.parse(value) do
      {parsed, ""} when parsed > 0 -> parsed
      _ -> nil
    end
  end

  defp normalize_optional_integer(_), do: nil

  defp maybe_put_if_absent(map, _key, nil), do: map

  defp maybe_put_if_absent(map, key, value) do
    if Map.has_key?(map, key), do: map, else: Map.put(map, key, value)
  end

  defp maybe_add_error(errors, true, message), do: [message | errors]
  defp maybe_add_error(errors, false, _message), do: errors

  defp validation_error(conn, status, errors) do
    conn
    |> put_status(status)
    |> json(%{
      "error" => %{
        "code" => "invalid_request",
        "message" => "tool invocation validation failed",
        "details" => errors
      }
    })
  end

  defp error(conn, status, code, message) do
    conn
    |> put_status(status)
    |> json(%{"error" => %{"code" => code, "message" => message}})
  end
end
