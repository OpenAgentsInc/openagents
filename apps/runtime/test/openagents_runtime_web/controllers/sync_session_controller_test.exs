defmodule OpenAgentsRuntimeWeb.SyncSessionControllerTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: false

  alias OpenAgentsRuntime.Sync.SessionRevocation

  setup do
    SessionRevocation.reset_for_tests()
    :ok
  end

  test "revoke marks session revocation records", %{conn: conn} do
    conn =
      conn
      |> put_internal_auth(user_id: 42)
      |> post(~p"/internal/v1/sync/sessions/revoke", %{
        "session_ids" => ["sess-revoke-1"],
        "reason" => "user_requested"
      })

    assert %{
             "data" => %{
               "revoked_session_ids" => ["sess-revoke-1"],
               "revoked_device_ids" => [],
               "reason" => "user_requested",
               "revoked_at" => revoked_at
             }
           } = json_response(conn, 200)

    assert is_integer(revoked_at)
    assert {:revoked, "user_requested"} = SessionRevocation.revoked?("sess-revoke-1", nil)
  end

  test "revoke validates missing identifiers", %{conn: conn} do
    conn =
      conn
      |> put_internal_auth(user_id: 42)
      |> post(~p"/internal/v1/sync/sessions/revoke", %{})

    assert %{"error" => %{"code" => "invalid_request"}} = json_response(conn, 400)
  end
end
