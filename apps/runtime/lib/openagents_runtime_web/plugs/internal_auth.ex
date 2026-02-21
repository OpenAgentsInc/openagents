defmodule OpenAgentsRuntimeWeb.Plugs.InternalAuth do
  @moduledoc """
  Enforces signed internal auth for all internal runtime endpoints.
  """

  @behaviour Plug

  import Plug.Conn

  alias OpenAgentsRuntime.Integrations.AuthTokenVerifier

  @signature_header "x-oa-runtime-signature"

  @impl Plug
  def init(opts), do: opts

  @impl Plug
  def call(conn, _opts) do
    token = conn |> get_req_header(@signature_header) |> List.first()

    case AuthTokenVerifier.verify(token, expected_claims: expected_claims(conn)) do
      :ok ->
        conn

      {:error, reason} ->
        %{code: code, message: message} = AuthTokenVerifier.error_details(reason)
        status = if code == "forbidden", do: 403, else: 401

        body = %{"error" => %{"code" => code, "message" => message}} |> Jason.encode!()

        conn
        |> put_resp_content_type("application/json")
        |> send_resp(status, body)
        |> halt()
    end
  end

  defp expected_claims(conn) do
    %{}
    |> maybe_put(:run_id, conn.path_params["run_id"] || conn.params["run_id"])
    |> maybe_put(:thread_id, conn.params["thread_id"])
    |> maybe_put(:user_id, parse_int_header(conn, "x-oa-user-id"))
    |> maybe_put(:guest_scope, header(conn, "x-oa-guest-scope"))
  end

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)

  defp parse_int_header(conn, header_name) do
    case header(conn, header_name) do
      nil ->
        nil

      value ->
        case Integer.parse(value) do
          {parsed, ""} -> parsed
          _ -> nil
        end
    end
  end

  defp header(conn, header_name) do
    conn
    |> get_req_header(header_name)
    |> List.first()
  end
end
