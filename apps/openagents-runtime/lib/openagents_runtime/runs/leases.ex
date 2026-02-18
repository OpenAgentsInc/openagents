defmodule OpenAgentsRuntime.Runs.Leases do
  @moduledoc """
  Run lease lifecycle with safe steal semantics.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.RunLease

  @type acquire_error :: :run_not_found | :lease_held | :lease_progressed | Ecto.Changeset.t()
  @type acquire_opts ::
          {:ttl_seconds, pos_integer()}
          | {:now, DateTime.t()}
          | {:observed_progress_seq, non_neg_integer()}

  @spec acquire(String.t(), String.t(), [acquire_opts()]) ::
          {:ok, RunLease.t()} | {:error, acquire_error()}
  def acquire(run_id, lease_owner, opts \\ [])
      when is_binary(run_id) and is_binary(lease_owner) and byte_size(lease_owner) > 0 do
    ttl_seconds = Keyword.get(opts, :ttl_seconds, 30)
    now = Keyword.get(opts, :now, DateTime.utc_now())
    observed_progress_seq = Keyword.get(opts, :observed_progress_seq, 0)
    lease_expires_at = DateTime.add(now, ttl_seconds, :second)

    Repo.transaction(fn ->
      case lock_lease(run_id) do
        nil ->
          create_lease(run_id, lease_owner, lease_expires_at, now)

        %RunLease{} = lease ->
          cond do
            lease.lease_owner == lease_owner ->
              renew_locked_lease(lease, lease_expires_at, now)

            DateTime.compare(lease.lease_expires_at, now) == :gt ->
              Repo.rollback(:lease_held)

            lease.last_progress_seq > observed_progress_seq ->
              Repo.rollback(:lease_progressed)

            true ->
              steal_locked_lease(lease, lease_owner, lease_expires_at, now)
          end
      end
    end)
    |> normalize_transaction_result()
  end

  @spec renew(String.t(), String.t(), keyword()) ::
          {:ok, RunLease.t()} | {:error, :not_owner | :not_found}
  def renew(run_id, lease_owner, opts \\ []) do
    ttl_seconds = Keyword.get(opts, :ttl_seconds, 30)
    now = Keyword.get(opts, :now, DateTime.utc_now())
    lease_expires_at = DateTime.add(now, ttl_seconds, :second)

    query =
      from(lease in RunLease,
        where: lease.run_id == ^run_id and lease.lease_owner == ^lease_owner,
        update: [set: [heartbeat_at: ^now, lease_expires_at: ^lease_expires_at]],
        select: lease
      )

    case Repo.update_all(query, []) do
      {1, [%RunLease{} = lease]} -> {:ok, lease}
      {0, _} -> if get(run_id), do: {:error, :not_owner}, else: {:error, :not_found}
    end
  end

  @spec mark_progress(String.t(), String.t(), non_neg_integer()) ::
          {:ok, RunLease.t()} | {:error, :not_owner | :not_found}
  def mark_progress(run_id, lease_owner, seq) when is_integer(seq) and seq >= 0 do
    now = DateTime.utc_now()

    query =
      from(lease in RunLease,
        where:
          lease.run_id == ^run_id and lease.lease_owner == ^lease_owner and
            lease.last_progress_seq <= ^seq,
        update: [set: [last_progress_seq: ^seq, heartbeat_at: ^now]],
        select: lease
      )

    case Repo.update_all(query, []) do
      {1, [%RunLease{} = lease]} -> {:ok, lease}
      {0, _} -> if get(run_id), do: {:error, :not_owner}, else: {:error, :not_found}
    end
  end

  @spec get(String.t()) :: RunLease.t() | nil
  def get(run_id) do
    query = from(lease in RunLease, where: lease.run_id == ^run_id, limit: 1)
    Repo.one(query)
  end

  defp lock_lease(run_id) do
    query =
      from(lease in RunLease,
        where: lease.run_id == ^run_id,
        lock: "FOR UPDATE",
        limit: 1
      )

    Repo.one(query)
  end

  defp create_lease(run_id, lease_owner, lease_expires_at, now) do
    changeset =
      RunLease.changeset(%RunLease{}, %{
        run_id: run_id,
        lease_owner: lease_owner,
        lease_expires_at: lease_expires_at,
        last_progress_seq: 0,
        heartbeat_at: now
      })

    case Repo.insert(changeset) do
      {:ok, lease} -> lease
      {:error, %Ecto.Changeset{} = changeset} -> Repo.rollback(changeset)
    end
  end

  defp renew_locked_lease(lease, lease_expires_at, now) do
    changeset =
      Ecto.Changeset.change(lease,
        lease_expires_at: lease_expires_at,
        heartbeat_at: now
      )

    case Repo.update(changeset) do
      {:ok, lease} -> lease
      {:error, %Ecto.Changeset{} = changeset} -> Repo.rollback(changeset)
    end
  end

  defp steal_locked_lease(lease, lease_owner, lease_expires_at, now) do
    changeset =
      Ecto.Changeset.change(lease,
        lease_owner: lease_owner,
        lease_expires_at: lease_expires_at,
        heartbeat_at: now
      )

    case Repo.update(changeset) do
      {:ok, lease} -> lease
      {:error, %Ecto.Changeset{} = changeset} -> Repo.rollback(changeset)
    end
  end

  defp normalize_transaction_result({:ok, %RunLease{} = lease}), do: {:ok, lease}
  defp normalize_transaction_result({:error, :lease_held}), do: {:error, :lease_held}
  defp normalize_transaction_result({:error, :lease_progressed}), do: {:error, :lease_progressed}

  defp normalize_transaction_result({:error, %Ecto.Changeset{} = changeset}),
    do: {:error, changeset}

  defp normalize_transaction_result({:error, %Postgrex.Error{} = _error}),
    do: {:error, :run_not_found}

  defp normalize_transaction_result({:error, _}), do: {:error, :run_not_found}
end
