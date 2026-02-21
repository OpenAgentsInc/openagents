defmodule OpenAgentsRuntimeWeb.CommsControllerTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: false

  alias OpenAgentsRuntime.Comms.DeliveryEvent
  alias OpenAgentsRuntime.Repo

  test "record_delivery_event accepts first write", %{conn: conn} do
    conn =
      conn
      |> put_internal_auth()
      |> post(~p"/internal/v1/comms/delivery-events", base_payload())

    assert %{
             "eventId" => "resend_evt_1",
             "status" => "accepted",
             "idempotentReplay" => false
           } = json_response(conn, 202)

    assert %DeliveryEvent{} = Repo.get_by(DeliveryEvent, event_id: "resend_evt_1")
  end

  test "record_delivery_event returns idempotent replay for duplicate payload", %{conn: conn} do
    conn =
      conn
      |> put_internal_auth()
      |> post(~p"/internal/v1/comms/delivery-events", base_payload())

    assert json_response(conn, 202)

    replay_conn =
      build_conn()
      |> put_internal_auth()
      |> put_req_header("content-type", "application/json")
      |> post(~p"/internal/v1/comms/delivery-events", base_payload())

    assert %{
             "eventId" => "resend_evt_1",
             "status" => "accepted",
             "idempotentReplay" => true
           } = json_response(replay_conn, 200)
  end

  test "record_delivery_event returns conflict for payload mismatch", %{conn: conn} do
    conn =
      conn
      |> put_internal_auth()
      |> post(~p"/internal/v1/comms/delivery-events", base_payload())

    assert json_response(conn, 202)

    conflict_payload = put_in(base_payload(), ["payload", "rawType"], "email.bounced")

    conflict_conn =
      build_conn()
      |> put_internal_auth()
      |> put_req_header("content-type", "application/json")
      |> post(~p"/internal/v1/comms/delivery-events", conflict_payload)

    assert %{"error" => %{"code" => "conflict"}} = json_response(conflict_conn, 409)
  end

  test "record_delivery_event rejects invalid payload", %{conn: conn} do
    invalid = Map.delete(base_payload(), "event_id")

    conn =
      conn
      |> put_internal_auth()
      |> post(~p"/internal/v1/comms/delivery-events", invalid)

    assert %{"error" => %{"code" => "invalid_request"}} = json_response(conn, 400)
  end

  test "record_delivery_event requires internal auth", %{conn: conn} do
    conn = post(conn, ~p"/internal/v1/comms/delivery-events", base_payload())
    assert %{"error" => %{"code" => "unauthorized"}} = json_response(conn, 401)
  end

  defp base_payload do
    %{
      "event_id" => "resend_evt_1",
      "provider" => "resend",
      "delivery_state" => "delivered",
      "message_id" => "email_123",
      "integration_id" => "resend.primary",
      "recipient" => "user@example.com",
      "occurred_at" => "2026-02-19T18:00:00Z",
      "reason" => nil,
      "payload" => %{"rawType" => "email.delivered"}
    }
  end
end
