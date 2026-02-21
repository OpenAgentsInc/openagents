defmodule OpenAgentsRuntime.Runs.EventHashChain do
  @moduledoc """
  Hash-chain helpers and integrity verification for run event logs.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.RunEvent

  @type verify_error :: {:chain_broken, non_neg_integer(), String.t()}

  @spec hash_event(String.t(), non_neg_integer(), String.t(), map(), String.t() | nil) ::
          String.t()
  def hash_event(run_id, seq, event_type, payload, prev_hash)
      when is_binary(run_id) and is_integer(seq) and is_binary(event_type) and is_map(payload) do
    payload_digest =
      payload
      |> :erlang.term_to_binary()
      |> then(&:crypto.hash(:sha256, &1))
      |> Base.encode16(case: :lower)

    material = [run_id, Integer.to_string(seq), event_type, payload_digest, prev_hash || "root"]
    hash(material)
  end

  @spec verify_run(String.t()) ::
          {:ok, %{event_count: non_neg_integer(), head_hash: String.t() | nil}}
          | {:error, verify_error()}
  def verify_run(run_id) when is_binary(run_id) do
    events =
      from(event in RunEvent,
        where: event.run_id == ^run_id,
        order_by: [asc: event.seq]
      )
      |> Repo.all()

    do_verify(events, nil)
  end

  defp do_verify([], last_hash), do: {:ok, %{event_count: 0, head_hash: last_hash}}

  defp do_verify(events, _last_hash) do
    Enum.reduce_while(events, %{previous_hash: nil, count: 0}, fn event, acc ->
      expected_hash =
        hash_event(event.run_id, event.seq, event.event_type, event.payload, acc.previous_hash)

      cond do
        event.prev_hash != acc.previous_hash ->
          {:halt, {:error, {:chain_broken, event.seq, "prev_hash mismatch"}}}

        event.event_hash != expected_hash ->
          {:halt, {:error, {:chain_broken, event.seq, "event_hash mismatch"}}}

        true ->
          {:cont, %{previous_hash: event.event_hash, count: acc.count + 1}}
      end
    end)
    |> case do
      {:error, _} = error ->
        error

      %{previous_hash: previous_hash, count: count} ->
        {:ok, %{event_count: count, head_hash: previous_hash}}
    end
  end

  defp hash(parts) do
    parts
    |> Enum.join("|")
    |> then(&:crypto.hash(:sha256, &1))
    |> Base.encode16(case: :lower)
  end
end
