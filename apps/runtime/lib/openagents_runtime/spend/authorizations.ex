defmodule OpenAgentsRuntime.Spend.Authorizations do
  @moduledoc """
  Spend authorization persistence and deterministic resolution.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.DS.PolicyEvaluator
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunOwnership
  alias OpenAgentsRuntime.Spend.SpendAuthorization

  @policy_id "spend.authorization.v1"

  @type resolution :: %{
          authorization: SpendAuthorization.t(),
          budget: map(),
          policy: map(),
          resolved_at: DateTime.t(),
          scope: map()
        }

  @spec create(map()) :: {:ok, SpendAuthorization.t()} | {:error, Ecto.Changeset.t()}
  def create(attrs) when is_map(attrs) do
    attrs =
      attrs
      |> stringify_keys()
      |> Map.put_new("authorization_id", generated_authorization_id())
      |> Map.put_new("issued_at", DateTime.utc_now())

    %SpendAuthorization{}
    |> SpendAuthorization.changeset(attrs)
    |> Repo.insert()
  end

  @spec resolve_for_run(String.t(), keyword()) ::
          {:ok, resolution()}
          | {:error,
             :authorization_missing | :ownership_not_found | :run_not_found | :thread_mismatch}
          | {:error, {:policy_denied, resolution()}}
  def resolve_for_run(run_id, opts \\ []) when is_binary(run_id) do
    requested_thread_id = normalize_optional_scope(Keyword.get(opts, :thread_id))
    autopilot_id = normalize_optional_scope(Keyword.get(opts, :autopilot_id))
    now = Keyword.get(opts, :now, DateTime.utc_now())

    with %Run{} = run <- Repo.get(Run, run_id),
         :ok <- validate_thread_scope(run, requested_thread_id),
         {:ok, owner_filter} <- owner_filter(run.run_id, run.thread_id),
         {:ok, authorization} <-
           resolve_candidate(owner_filter, run.run_id, run.thread_id, autopilot_id) do
      evaluate_authorization(authorization, now)
    else
      nil -> {:error, :run_not_found}
      {:error, _} = error -> error
    end
  end

  @spec build_policy(SpendAuthorization.t()) :: map()
  def build_policy(%SpendAuthorization{} = authorization) do
    %{
      "policy_id" => @policy_id,
      "authorization_id" => authorization.authorization_id,
      "authorization_mode" => authorization.mode
    }
  end

  @spec budget_snapshot(SpendAuthorization.t()) :: map()
  def budget_snapshot(%SpendAuthorization{} = authorization) do
    spent = normalize_non_negative(authorization.spent_sats)
    reserved = normalize_non_negative(authorization.reserved_sats)
    max_total = normalize_optional_non_negative(authorization.max_total_sats)

    remaining =
      case max_total do
        nil -> nil
        limit -> max(limit - spent - reserved, 0)
      end

    %{
      "max_total_sats" => max_total,
      "max_per_call_sats" => normalize_optional_non_negative(authorization.max_per_call_sats),
      "max_per_day_sats" => normalize_optional_non_negative(authorization.max_per_day_sats),
      "threshold_sats" => normalize_optional_non_negative(authorization.threshold_sats),
      "spent_sats" => spent,
      "reserved_sats" => reserved,
      "remaining_sats" => remaining
    }
  end

  defp evaluate_authorization(%SpendAuthorization{} = authorization, now) do
    policy = build_policy(authorization)
    budget = budget_snapshot(authorization)

    cond do
      revoked?(authorization, now) ->
        deny_resolution(authorization, now, policy, budget, "policy_denied.authorization_revoked")

      expired?(authorization, now) ->
        deny_resolution(authorization, now, policy, budget, "policy_denied.authorization_expired")

      authorization.mode == "deny" ->
        deny_resolution(authorization, now, policy, budget, "policy_denied.explicit_deny")

      true ->
        evaluation = PolicyEvaluator.evaluate(policy, budget, %{})
        resolution = build_resolution(authorization, now, budget, evaluation)

        if evaluation["decision"] == "allowed" do
          {:ok, resolution}
        else
          {:error, {:policy_denied, resolution}}
        end
    end
  end

  defp deny_resolution(authorization, now, policy, budget, reason_code) do
    evaluation =
      policy
      |> Map.put("decision", "denied")
      |> Map.put("reason_code", reason_code)
      |> PolicyEvaluator.evaluate(budget, %{})

    {:error, {:policy_denied, build_resolution(authorization, now, budget, evaluation)}}
  end

  defp build_resolution(authorization, now, budget, policy) do
    %{
      authorization: authorization,
      budget: budget,
      policy: policy,
      resolved_at: now,
      scope: %{
        "run_id" => authorization.run_id,
        "thread_id" => authorization.thread_id,
        "autopilot_id" => authorization.autopilot_id
      }
    }
  end

  defp owner_filter(run_id, thread_id) do
    query =
      from(ownership in RunOwnership,
        where: ownership.run_id == ^run_id and ownership.thread_id == ^thread_id,
        limit: 1
      )

    case Repo.one(query) do
      %RunOwnership{user_id: user_id} when is_integer(user_id) ->
        {:ok, {:owner_user_id, user_id}}

      %RunOwnership{guest_scope: guest_scope}
      when is_binary(guest_scope) and byte_size(guest_scope) > 0 ->
        {:ok, {:owner_guest_scope, guest_scope}}

      _ ->
        {:error, :ownership_not_found}
    end
  end

  defp resolve_candidate(owner_filter, run_id, thread_id, autopilot_id) do
    query =
      SpendAuthorization
      |> where(^owner_condition(owner_filter))
      |> where([authorization], is_nil(authorization.run_id) or authorization.run_id == ^run_id)
      |> where(
        [authorization],
        is_nil(authorization.thread_id) or authorization.thread_id == ^thread_id
      )
      |> maybe_filter_autopilot(autopilot_id)
      |> order_by(
        [authorization],
        desc: fragment("CASE WHEN ? IS NULL THEN 0 ELSE 1 END", authorization.run_id),
        desc: fragment("CASE WHEN ? IS NULL THEN 0 ELSE 1 END", authorization.thread_id),
        desc: fragment("CASE WHEN ? IS NULL THEN 0 ELSE 1 END", authorization.autopilot_id),
        desc: authorization.inserted_at,
        desc: authorization.authorization_id
      )
      |> limit(1)

    case Repo.one(query) do
      %SpendAuthorization{} = authorization -> {:ok, authorization}
      nil -> {:error, :authorization_missing}
    end
  end

  defp owner_condition({:owner_user_id, user_id}) do
    dynamic(
      [authorization],
      authorization.owner_user_id == ^user_id and is_nil(authorization.owner_guest_scope)
    )
  end

  defp owner_condition({:owner_guest_scope, guest_scope}) do
    dynamic(
      [authorization],
      authorization.owner_guest_scope == ^guest_scope and is_nil(authorization.owner_user_id)
    )
  end

  defp maybe_filter_autopilot(query, nil) do
    where(query, [authorization], is_nil(authorization.autopilot_id))
  end

  defp maybe_filter_autopilot(query, autopilot_id) do
    where(
      query,
      [authorization],
      is_nil(authorization.autopilot_id) or authorization.autopilot_id == ^autopilot_id
    )
  end

  defp validate_thread_scope(_run, nil), do: :ok

  defp validate_thread_scope(%Run{thread_id: thread_id}, requested_thread_id)
       when is_binary(thread_id) and is_binary(requested_thread_id) do
    if thread_id == requested_thread_id, do: :ok, else: {:error, :thread_mismatch}
  end

  defp revoked?(%SpendAuthorization{revoked_at: nil}, _now), do: false

  defp revoked?(%SpendAuthorization{revoked_at: revoked_at}, now) do
    DateTime.compare(revoked_at, now) in [:lt, :eq]
  end

  defp expired?(%SpendAuthorization{expires_at: nil}, _now), do: false

  defp expired?(%SpendAuthorization{expires_at: expires_at}, now) do
    DateTime.compare(expires_at, now) in [:lt, :eq]
  end

  defp generated_authorization_id do
    "auth_" <> String.replace(Ecto.UUID.generate(), "-", "")
  end

  defp normalize_optional_scope(value) when is_binary(value) do
    case String.trim(value) do
      "" -> nil
      normalized -> normalized
    end
  end

  defp normalize_optional_scope(_), do: nil

  defp normalize_non_negative(value) when is_integer(value) and value >= 0, do: value
  defp normalize_non_negative(_), do: 0

  defp normalize_optional_non_negative(value) when is_integer(value) and value >= 0, do: value
  defp normalize_optional_non_negative(_), do: nil

  defp stringify_keys(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end
end
