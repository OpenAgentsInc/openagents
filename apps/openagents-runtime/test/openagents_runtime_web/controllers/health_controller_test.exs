defmodule OpenAgentsRuntimeWeb.HealthControllerTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: true

  test "GET /internal/v1/health returns runtime status", %{conn: conn} do
    conn =
      conn
      |> put_internal_auth()
      |> get(~p"/internal/v1/health")

    assert %{
             "status" => "ok",
             "service" => "openagents-runtime",
             "version" => _
           } = json_response(conn, 200)
  end
end
