defmodule OpenAgentsRuntime.Runs.OwnershipGuard do
  @moduledoc """
  Validates run/thread ownership from DB records.

  Runtime never trusts principal ownership claims from request payloads.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.RunOwnership

  @typedoc """
  Principal for ownership checks.

  Requires at least one of:
  - `:user_id`
  - `:guest_scope`
  """
  @type principal :: %{optional(:user_id) => integer(), optional(:guest_scope) => String.t()}

  @spec authorize(String.t(), String.t(), principal()) ::
          :ok | {:error, :invalid_principal | :not_found | :forbidden}
  def authorize(run_id, thread_id, principal)
      when is_binary(run_id) and is_binary(thread_id) and is_map(principal) do
    with {:ok, principal} <- normalize_principal(principal),
         {:ok, ownership} <- load_ownership(run_id, thread_id),
         true <- ownership_matches?(ownership, principal) do
      :ok
    else
      {:error, _} = error -> error
      false -> {:error, :forbidden}
    end
  end

  def authorize(_run_id, _thread_id, _principal), do: {:error, :invalid_principal}

  @spec normalize_principal(map()) :: {:ok, principal()} | {:error, :invalid_principal}
  def normalize_principal(%{user_id: user_id} = principal) when is_integer(user_id) do
    {:ok, Map.take(principal, [:user_id, :guest_scope])}
  end

  def normalize_principal(%{guest_scope: guest_scope} = principal) when is_binary(guest_scope) do
    {:ok, Map.take(principal, [:user_id, :guest_scope])}
  end

  def normalize_principal(_), do: {:error, :invalid_principal}

  defp load_ownership(run_id, thread_id) do
    query =
      from(ownership in RunOwnership,
        where: ownership.run_id == ^run_id and ownership.thread_id == ^thread_id,
        limit: 1
      )

    case Repo.one(query) do
      %RunOwnership{} = ownership -> {:ok, ownership}
      nil -> {:error, :not_found}
    end
  end

  defp ownership_matches?(ownership, principal) do
    cond do
      is_integer(principal[:user_id]) -> ownership.user_id == principal.user_id
      is_binary(principal[:guest_scope]) -> ownership.guest_scope == principal.guest_scope
      true -> false
    end
  end
end
