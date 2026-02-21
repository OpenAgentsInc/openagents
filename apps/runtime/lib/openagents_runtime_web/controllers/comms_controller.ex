defmodule OpenAgentsRuntimeWeb.CommsController do
  use OpenAgentsRuntimeWeb, :controller

  alias OpenAgentsRuntime.Comms.DeliveryEvents
  alias OpenAgentsRuntime.Telemetry.Tracing

  @spec record_delivery_event(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def record_delivery_event(conn, params) when is_map(params) do
    tracing_context = %{
      provider: params["provider"],
      event_id: params["event_id"],
      delivery_state: params["delivery_state"]
    }

    Tracing.with_phase_span(:ingest, tracing_context, fn ->
      with {:ok, result} <- DeliveryEvents.ingest(params) do
        status = if result.idempotent_replay, do: 200, else: 202

        conn
        |> put_status(status)
        |> json(%{
          "eventId" => result.event.event_id,
          "status" => "accepted",
          "idempotentReplay" => result.idempotent_replay
        })
      else
        {:error, :idempotency_conflict} ->
          error(conn, 409, "conflict", "event_id payload mismatch for existing webhook event")

        {:error, %Ecto.Changeset{} = changeset} ->
          error(conn, 400, "invalid_request", inspect(changeset.errors))
      end
    end)
  end

  defp error(conn, status, code, message) do
    conn
    |> put_status(status)
    |> json(%{"error" => %{"code" => code, "message" => message}})
  end
end
