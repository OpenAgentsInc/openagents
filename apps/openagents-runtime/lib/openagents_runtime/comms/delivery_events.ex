defmodule OpenAgentsRuntime.Comms.DeliveryEvents do
  @moduledoc """
  Idempotent ingestion for normalized comms delivery events.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Comms.DeliveryEvent
  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.Repo

  @type ingest_result :: %{event: DeliveryEvent.t(), idempotent_replay: boolean()}
  @type ingest_error :: :idempotency_conflict | Ecto.Changeset.t()

  @spec ingest(map()) :: {:ok, ingest_result()} | {:error, ingest_error()}
  def ingest(attrs) when is_map(attrs) do
    event_id = attrs[:event_id] || attrs["event_id"]
    provider = attrs[:provider] || attrs["provider"]
    delivery_state = attrs[:delivery_state] || attrs["delivery_state"]
    message_id = attrs[:message_id] || attrs["message_id"]
    integration_id = attrs[:integration_id] || attrs["integration_id"]
    recipient = attrs[:recipient] || attrs["recipient"]
    occurred_at = attrs[:occurred_at] || attrs["occurred_at"]
    reason = attrs[:reason] || attrs["reason"]
    payload = attrs[:payload] || attrs["payload"] || %{}

    payload_hash = Receipts.stable_hash(payload)

    changeset =
      DeliveryEvent.changeset(%DeliveryEvent{}, %{
        event_id: event_id,
        provider: provider,
        delivery_state: delivery_state,
        message_id: message_id,
        integration_id: integration_id,
        recipient: recipient,
        occurred_at: occurred_at,
        reason: reason,
        payload: payload,
        payload_hash: payload_hash
      })

    case Repo.insert(changeset,
           on_conflict: :nothing,
           conflict_target: [:event_id],
           returning: true
         ) do
      {:ok, %DeliveryEvent{id: nil}} ->
        resolve_existing_event(event_id, payload_hash)

      {:ok, %DeliveryEvent{} = event} ->
        {:ok, %{event: event, idempotent_replay: false}}

      {:error, %Ecto.Changeset{} = changeset} ->
        {:error, changeset}
    end
  end

  @spec get_by_event_id(String.t()) :: DeliveryEvent.t() | nil
  def get_by_event_id(event_id) when is_binary(event_id) do
    query = from(event in DeliveryEvent, where: event.event_id == ^event_id, limit: 1)
    Repo.one(query)
  end

  defp resolve_existing_event(event_id, payload_hash) when is_binary(event_id) do
    case get_by_event_id(event_id) do
      %DeliveryEvent{} = event ->
        if event.payload_hash == payload_hash do
          {:ok, %{event: event, idempotent_replay: true}}
        else
          {:error, :idempotency_conflict}
        end

      nil ->
        {:error, :idempotency_conflict}
    end
  end

  defp resolve_existing_event(_event_id, _payload_hash), do: {:error, :idempotency_conflict}
end
