defmodule OpenAgentsRuntime.Spend.AuthorizationsTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunOwnership
  alias OpenAgentsRuntime.Spend.Authorizations

  setup do
    run_id = unique_id("run_spend")
    thread_id = unique_id("thread_spend")
    owner_user_id = System.unique_integer([:positive])

    insert_run_with_ownership(run_id, thread_id, owner_user_id)

    {:ok, run_id: run_id, thread_id: thread_id, owner_user_id: owner_user_id}
  end

  test "resolves most specific authorization by scope precedence", ctx do
    _global =
      insert_authorization!(%{
        owner_user_id: ctx.owner_user_id,
        max_total_sats: 10_000
      })

    _thread =
      insert_authorization!(%{
        owner_user_id: ctx.owner_user_id,
        thread_id: ctx.thread_id,
        max_total_sats: 8_000
      })

    run_scope =
      insert_authorization!(%{
        owner_user_id: ctx.owner_user_id,
        run_id: ctx.run_id,
        max_total_sats: 6_000
      })

    assert {:ok, resolution} = Authorizations.resolve_for_run(ctx.run_id)
    assert resolution.authorization.authorization_id == run_scope.authorization_id
    assert resolution.policy["decision"] == "allowed"
    assert resolution.policy["reason_code"] == "policy_allowed.default"
    assert resolution.budget["remaining_sats"] == 6_000
  end

  test "expired authorization is denied deterministically without lower-scope fallback", ctx do
    _global =
      insert_authorization!(%{
        owner_user_id: ctx.owner_user_id,
        max_total_sats: 20_000
      })

    expired =
      insert_authorization!(%{
        owner_user_id: ctx.owner_user_id,
        run_id: ctx.run_id,
        max_total_sats: 1_500,
        expires_at: DateTime.add(DateTime.utc_now(), -30, :second)
      })

    assert {:error, {:policy_denied, resolution}} = Authorizations.resolve_for_run(ctx.run_id)
    assert resolution.authorization.authorization_id == expired.authorization_id
    assert resolution.policy["decision"] == "denied"
    assert resolution.policy["reason_code"] == "policy_denied.authorization_expired"
  end

  test "deny mode is rejected with explicit policy outcome", ctx do
    deny_scope =
      insert_authorization!(%{
        owner_user_id: ctx.owner_user_id,
        run_id: ctx.run_id,
        mode: "deny"
      })

    assert {:error, {:policy_denied, resolution}} = Authorizations.resolve_for_run(ctx.run_id)
    assert resolution.authorization.authorization_id == deny_scope.authorization_id
    assert resolution.policy["decision"] == "denied"
    assert resolution.policy["reason_code"] == "policy_denied.explicit_deny"
  end

  test "resolution uses DB ownership and ignores authorizations for other principals", ctx do
    _wrong_owner =
      insert_authorization!(%{
        owner_user_id: ctx.owner_user_id + 100,
        run_id: ctx.run_id,
        max_total_sats: 999
      })

    assert {:error, :authorization_missing} = Authorizations.resolve_for_run(ctx.run_id)

    valid =
      insert_authorization!(%{
        owner_user_id: ctx.owner_user_id,
        run_id: ctx.run_id,
        max_total_sats: 500
      })

    assert {:ok, resolution} = Authorizations.resolve_for_run(ctx.run_id)
    assert resolution.authorization.authorization_id == valid.authorization_id
  end

  test "thread_id mismatch is rejected", ctx do
    insert_authorization!(%{
      owner_user_id: ctx.owner_user_id,
      run_id: ctx.run_id
    })

    assert {:error, :thread_mismatch} =
             Authorizations.resolve_for_run(ctx.run_id, thread_id: "wrong-thread")
  end

  defp insert_authorization!(attrs) do
    defaults = %{
      mode: "delegated_budget",
      spent_sats: 0,
      reserved_sats: 0,
      constraints: %{},
      metadata: %{}
    }

    attrs
    |> Map.merge(defaults, fn _key, override, _default -> override end)
    |> then(fn payload ->
      case Authorizations.create(payload) do
        {:ok, authorization} -> authorization
        {:error, changeset} -> flunk("authorization insert failed: #{inspect(changeset.errors)}")
      end
    end)
  end

  defp insert_run_with_ownership(run_id, thread_id, owner_user_id) do
    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "created",
      owner_user_id: owner_user_id,
      latest_seq: 0
    })

    Repo.insert!(%RunOwnership{
      run_id: run_id,
      thread_id: thread_id,
      user_id: owner_user_id
    })
  end

  defp unique_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end
end
