defmodule OpenAgentsRuntime.Comms.DeliveryEventsTest do
  use OpenAgentsRuntime.DataCase, async: true

  alias OpenAgentsRuntime.Comms.DeliveryEvents

  test "ingest/1 accepts first write" do
    assert {:ok, %{idempotent_replay: false, event: event}} =
             DeliveryEvents.ingest(base_event())

    assert event.event_id == "resend_evt_1"
    assert event.provider == "resend"
    assert event.delivery_state == "delivered"
  end

  test "ingest/1 returns idempotent replay for duplicate payload" do
    assert {:ok, %{idempotent_replay: false}} = DeliveryEvents.ingest(base_event())

    assert {:ok, %{idempotent_replay: true, event: event}} =
             DeliveryEvents.ingest(base_event())

    assert event.event_id == "resend_evt_1"
  end

  test "ingest/1 returns conflict for duplicate event_id with changed payload" do
    assert {:ok, %{idempotent_replay: false}} = DeliveryEvents.ingest(base_event())

    conflicting = put_in(base_event(), ["payload", "rawType"], "email.bounced")

    assert {:error, :idempotency_conflict} = DeliveryEvents.ingest(conflicting)
  end

  test "ingest/1 returns changeset for invalid delivery state" do
    invalid = Map.put(base_event(), "delivery_state", "opened")

    assert {:error, %Ecto.Changeset{} = changeset} = DeliveryEvents.ingest(invalid)
    assert "is invalid" in errors_on(changeset).delivery_state
  end

  defp base_event do
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
