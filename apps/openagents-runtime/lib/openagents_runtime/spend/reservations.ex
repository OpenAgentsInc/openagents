defmodule OpenAgentsRuntime.Spend.Reservations do
  @moduledoc """
  Atomic reserve/commit/release/reconcile operations for spend reservations.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Spend.SpendAuthorization
  alias OpenAgentsRuntime.Spend.SpendReservation

  @retry_classes MapSet.new(SpendReservation.retry_classes())

  @type op_result :: %{
          reservation: SpendReservation.t(),
          authorization: SpendAuthorization.t(),
          idempotent_replay: boolean()
        }

  @spec reserve(String.t(), String.t(), String.t(), pos_integer(), keyword()) ::
          {:ok, op_result()}
          | {:error,
             :already_finalized
             | :authorization_not_found
             | :invalid_retry_class
             | :idempotency_conflict
             | :invalid_amount
             | :over_budget
             | :reconcile_required}
  def reserve(authorization_id, run_id, tool_call_id, amount_sats, opts \\ [])

  def reserve(authorization_id, run_id, tool_call_id, amount_sats, opts)
      when is_binary(authorization_id) and is_binary(run_id) and is_binary(tool_call_id) do
    if is_integer(amount_sats) and amount_sats > 0 do
      now = Keyword.get(opts, :now, DateTime.utc_now())
      metadata = normalize_map(Keyword.get(opts, :metadata, %{}))
      retry_class = normalize_retry_class(Keyword.get(opts, :retry_class, "safe_retry"))

      provider_idempotency_key =
        normalize_optional_string(Keyword.get(opts, :provider_idempotency_key))

      if MapSet.member?(@retry_classes, retry_class) do
        Repo.transaction(fn ->
          authorization = lock_authorization!(authorization_id)

          case lock_reservation(authorization_id, run_id, tool_call_id) do
            %SpendReservation{} = reservation ->
              ensure_amount_matches!(reservation, amount_sats)

              case reservation.state do
                "reserved" ->
                  %{
                    reservation: reservation,
                    authorization: authorization,
                    idempotent_replay: true
                  }

                "reconcile_required" when retry_class == "dedupe_reconcile_required" ->
                  Repo.rollback(:reconcile_required)

                "reconcile_required" ->
                  %{
                    reservation: reservation,
                    authorization: authorization,
                    idempotent_replay: true
                  }

                "committed" ->
                  Repo.rollback(:already_finalized)

                "released" ->
                  Repo.rollback(:already_finalized)

                _ ->
                  Repo.rollback(:invalid_transition)
              end

            nil ->
              ensure_budget!(authorization, amount_sats)

              reservation =
                insert_reservation!(%{
                  authorization_id: authorization_id,
                  run_id: run_id,
                  tool_call_id: tool_call_id,
                  amount_sats: amount_sats,
                  state: "reserved",
                  retry_class: retry_class,
                  metadata: metadata,
                  provider_idempotency_key: provider_idempotency_key,
                  reserved_at: now
                })

              authorization =
                update_authorization!(authorization, %{
                  reserved_sats: normalize_int(authorization.reserved_sats) + amount_sats
                })

              %{reservation: reservation, authorization: authorization, idempotent_replay: false}
          end
        end)
        |> unwrap_transaction()
      else
        {:error, :invalid_retry_class}
      end
    else
      {:error, :invalid_amount}
    end
  end

  def reserve(_authorization_id, _run_id, _tool_call_id, _amount_sats, _opts),
    do: {:error, :invalid_amount}

  @spec commit(String.t(), String.t(), String.t(), keyword()) ::
          {:ok, op_result()}
          | {:error,
             :authorization_not_found
             | :invalid_transition
             | :reservation_not_found
             | :idempotency_conflict}
  def commit(authorization_id, run_id, tool_call_id, opts \\ [])
      when is_binary(authorization_id) and is_binary(run_id) and is_binary(tool_call_id) do
    now = Keyword.get(opts, :now, DateTime.utc_now())
    metadata = normalize_map(Keyword.get(opts, :metadata, %{}))
    provider_correlation_id = Keyword.get(opts, :provider_correlation_id)
    provider_idempotency_key = Keyword.get(opts, :provider_idempotency_key)
    reconciled = Keyword.get(opts, :reconciled, false)

    Repo.transaction(fn ->
      authorization = lock_authorization!(authorization_id)
      reservation = lock_reservation!(authorization_id, run_id, tool_call_id)

      case reservation.state do
        "committed" ->
          %{reservation: reservation, authorization: authorization, idempotent_replay: true}

        "reserved" ->
          apply_commit!(
            reservation,
            authorization,
            now,
            metadata,
            provider_correlation_id,
            provider_idempotency_key,
            reconciled
          )

        "reconcile_required" ->
          apply_commit!(
            reservation,
            authorization,
            now,
            metadata,
            provider_correlation_id,
            provider_idempotency_key,
            true
          )

        "released" ->
          Repo.rollback(:invalid_transition)

        _ ->
          Repo.rollback(:invalid_transition)
      end
    end)
    |> unwrap_transaction()
  end

  @spec release(String.t(), String.t(), String.t(), keyword()) ::
          {:ok, op_result()}
          | {:error,
             :authorization_not_found
             | :idempotency_conflict
             | :invalid_transition
             | :reservation_not_found}
  def release(authorization_id, run_id, tool_call_id, opts \\ [])
      when is_binary(authorization_id) and is_binary(run_id) and is_binary(tool_call_id) do
    now = Keyword.get(opts, :now, DateTime.utc_now())
    metadata = normalize_map(Keyword.get(opts, :metadata, %{}))
    failure_reason = Keyword.get(opts, :failure_reason)
    reconciled = Keyword.get(opts, :reconciled, false)

    Repo.transaction(fn ->
      authorization = lock_authorization!(authorization_id)
      reservation = lock_reservation!(authorization_id, run_id, tool_call_id)

      case reservation.state do
        "released" ->
          %{reservation: reservation, authorization: authorization, idempotent_replay: true}

        "reserved" ->
          apply_release!(reservation, authorization, now, metadata, failure_reason, reconciled)

        "reconcile_required" ->
          apply_release!(reservation, authorization, now, metadata, failure_reason, true)

        "committed" ->
          Repo.rollback(:invalid_transition)

        _ ->
          Repo.rollback(:invalid_transition)
      end
    end)
    |> unwrap_transaction()
  end

  @spec mark_reconcile_required(String.t(), String.t(), String.t(), keyword()) ::
          {:ok, op_result()}
          | {:error,
             :authorization_not_found
             | :idempotency_conflict
             | :invalid_transition
             | :reservation_not_found}
  def mark_reconcile_required(authorization_id, run_id, tool_call_id, opts \\ [])
      when is_binary(authorization_id) and is_binary(run_id) and is_binary(tool_call_id) do
    now = Keyword.get(opts, :now, DateTime.utc_now())
    metadata = normalize_map(Keyword.get(opts, :metadata, %{}))
    failure_reason = Keyword.get(opts, :failure_reason, "unknown_settlement_outcome")

    Repo.transaction(fn ->
      authorization = lock_authorization!(authorization_id)
      reservation = lock_reservation!(authorization_id, run_id, tool_call_id)

      case reservation.state do
        "reconcile_required" ->
          %{reservation: reservation, authorization: authorization, idempotent_replay: true}

        "reserved" ->
          reservation =
            reservation
            |> Ecto.Changeset.change(%{
              state: "reconcile_required",
              metadata: Map.merge(reservation.metadata || %{}, metadata),
              failure_reason: failure_reason,
              reconciled_at: now
            })
            |> Repo.update!()

          %{reservation: reservation, authorization: authorization, idempotent_replay: false}

        state when state in ["committed", "released"] ->
          Repo.rollback(:invalid_transition)

        _ ->
          Repo.rollback(:invalid_transition)
      end
    end)
    |> unwrap_transaction()
  end

  @spec reconcile(String.t(), String.t(), String.t(), :commit | :release, keyword()) ::
          {:ok, op_result()}
          | {:error,
             :authorization_not_found
             | :idempotency_conflict
             | :invalid_transition
             | :reservation_not_found}
  def reconcile(authorization_id, run_id, tool_call_id, outcome, opts \\ [])
      when outcome in [:commit, :release] do
    case outcome do
      :commit ->
        commit(authorization_id, run_id, tool_call_id, Keyword.put(opts, :reconciled, true))

      :release ->
        release(authorization_id, run_id, tool_call_id, Keyword.put(opts, :reconciled, true))
    end
  end

  @spec recover_stuck(keyword()) :: {:ok, [op_result()]}
  def recover_stuck(opts \\ []) do
    stale_before =
      Keyword.get(opts, :stale_before, DateTime.add(DateTime.utc_now(), -60, :second))

    query =
      from(reservation in SpendReservation,
        where: reservation.state == "reserved" and reservation.reserved_at <= ^stale_before,
        order_by: [asc: reservation.reserved_at]
      )

    query
    |> Repo.all()
    |> Enum.reduce([], fn reservation, acc ->
      case mark_reconcile_required(
             reservation.authorization_id,
             reservation.run_id,
             reservation.tool_call_id,
             failure_reason: "stale_reservation"
           ) do
        {:ok, result} -> [result | acc]
        {:error, _reason} -> acc
      end
    end)
    |> Enum.reverse()
    |> then(&{:ok, &1})
  end

  @spec get(String.t(), String.t(), String.t()) :: SpendReservation.t() | nil
  def get(authorization_id, run_id, tool_call_id)
      when is_binary(authorization_id) and is_binary(run_id) and is_binary(tool_call_id) do
    query =
      from(reservation in SpendReservation,
        where:
          reservation.authorization_id == ^authorization_id and reservation.run_id == ^run_id and
            reservation.tool_call_id == ^tool_call_id,
        limit: 1
      )

    Repo.one(query)
  end

  defp apply_commit!(
         reservation,
         authorization,
         now,
         metadata,
         provider_correlation_id,
         provider_idempotency_key,
         reconciled
       ) do
    reservation =
      reservation
      |> Ecto.Changeset.change(%{
        state: "committed",
        metadata: Map.merge(reservation.metadata || %{}, metadata),
        committed_at: now,
        provider_correlation_id: provider_correlation_id || reservation.provider_correlation_id,
        provider_idempotency_key:
          provider_idempotency_key || reservation.provider_idempotency_key,
        reconciled_at: if(reconciled, do: now, else: reservation.reconciled_at)
      })
      |> Repo.update!()

    authorization =
      update_authorization!(authorization, %{
        spent_sats: normalize_int(authorization.spent_sats) + reservation.amount_sats,
        reserved_sats:
          max(normalize_int(authorization.reserved_sats) - reservation.amount_sats, 0)
      })

    %{reservation: reservation, authorization: authorization, idempotent_replay: false}
  end

  defp apply_release!(reservation, authorization, now, metadata, failure_reason, reconciled) do
    reservation =
      reservation
      |> Ecto.Changeset.change(%{
        state: "released",
        metadata: Map.merge(reservation.metadata || %{}, metadata),
        failure_reason: failure_reason || reservation.failure_reason,
        released_at: now,
        reconciled_at: if(reconciled, do: now, else: reservation.reconciled_at)
      })
      |> Repo.update!()

    authorization =
      update_authorization!(authorization, %{
        reserved_sats:
          max(normalize_int(authorization.reserved_sats) - reservation.amount_sats, 0)
      })

    %{reservation: reservation, authorization: authorization, idempotent_replay: false}
  end

  defp ensure_amount_matches!(reservation, amount_sats) do
    if reservation.amount_sats == amount_sats do
      :ok
    else
      Repo.rollback(:idempotency_conflict)
    end
  end

  defp ensure_budget!(authorization, amount_sats) do
    max_total = authorization.max_total_sats
    spent = normalize_int(authorization.spent_sats)
    reserved = normalize_int(authorization.reserved_sats)

    budget_exceeded? =
      is_integer(max_total) and max_total >= 0 and spent + reserved + amount_sats > max_total

    if budget_exceeded? do
      Repo.rollback(:over_budget)
    else
      :ok
    end
  end

  defp lock_authorization!(authorization_id) do
    query =
      from(authorization in SpendAuthorization,
        where: authorization.authorization_id == ^authorization_id,
        lock: "FOR UPDATE",
        limit: 1
      )

    case Repo.one(query) do
      %SpendAuthorization{} = authorization -> authorization
      nil -> Repo.rollback(:authorization_not_found)
    end
  end

  defp lock_reservation!(authorization_id, run_id, tool_call_id) do
    case lock_reservation(authorization_id, run_id, tool_call_id) do
      %SpendReservation{} = reservation -> reservation
      nil -> Repo.rollback(:reservation_not_found)
    end
  end

  defp lock_reservation(authorization_id, run_id, tool_call_id) do
    query =
      from(reservation in SpendReservation,
        where:
          reservation.authorization_id == ^authorization_id and reservation.run_id == ^run_id and
            reservation.tool_call_id == ^tool_call_id,
        lock: "FOR UPDATE",
        limit: 1
      )

    Repo.one(query)
  end

  defp insert_reservation!(attrs) do
    %SpendReservation{}
    |> SpendReservation.changeset(attrs)
    |> Repo.insert!()
  end

  defp update_authorization!(authorization, attrs) do
    authorization
    |> Ecto.Changeset.change(attrs)
    |> Repo.update!()
  end

  defp unwrap_transaction({:ok, result}), do: {:ok, result}
  defp unwrap_transaction({:error, reason}), do: {:error, reason}

  defp normalize_map(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp normalize_map(_), do: %{}

  defp normalize_int(value) when is_integer(value) and value >= 0, do: value
  defp normalize_int(_), do: 0

  defp normalize_retry_class(value) when is_binary(value), do: String.trim(value)

  defp normalize_retry_class(value) when is_atom(value),
    do: value |> Atom.to_string() |> String.trim()

  defp normalize_retry_class(_), do: "safe_retry"

  defp normalize_optional_string(value) when is_binary(value) do
    case String.trim(value) do
      "" -> nil
      normalized -> normalized
    end
  end

  defp normalize_optional_string(_), do: nil
end
