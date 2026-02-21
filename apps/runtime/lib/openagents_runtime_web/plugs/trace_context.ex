defmodule OpenAgentsRuntimeWeb.Plugs.TraceContext do
  @moduledoc """
  Extracts and propagates W3C trace context for internal runtime requests.
  """

  @behaviour Plug

  alias OpenAgentsRuntime.Telemetry.Tracing

  @impl Plug
  def init(opts), do: opts

  @impl Plug
  def call(conn, _opts) do
    context =
      conn
      |> Tracing.extract_from_conn()
      |> ensure_request_id(conn)

    :ok = Tracing.put_current(context)

    conn
    |> Plug.Conn.put_private(:trace_context, context)
    |> Plug.Conn.register_before_send(fn conn ->
      maybe_put_response_header(conn, "x-request-id", Map.get(context, "x-request-id"))
    end)
  end

  defp ensure_request_id(context, conn) do
    case Map.get(context, "x-request-id") do
      value when is_binary(value) and value != "" ->
        context

      _ ->
        request_id = conn |> Plug.Conn.get_resp_header("x-request-id") |> List.first()

        if is_binary(request_id) and request_id != "" do
          Map.put(context, "x-request-id", request_id)
        else
          context
        end
    end
  end

  defp maybe_put_response_header(conn, _header, nil), do: conn

  defp maybe_put_response_header(conn, header, value) when is_binary(value) do
    case Plug.Conn.get_resp_header(conn, header) do
      [] -> Plug.Conn.put_resp_header(conn, header, value)
      _ -> conn
    end
  end
end
