defmodule OpenAgentsRuntime.DS.Compile.PromoteService do
  @moduledoc """
  Pointer-based DS artifact promotion and rollback with durable audit logs.
  """

  import Ecto.Query

  alias Ecto.Multi
  alias OpenAgentsRuntime.DS.PointerAudit
  alias OpenAgentsRuntime.DS.PolicyPointer
  alias OpenAgentsRuntime.Repo

  @type mutate_opt ::
          {:actor, String.t()}
          | {:reason, String.t()}
          | {:metadata, map()}
          | {:strategy_id, String.t()}
          | {:clear_canary, boolean()}
          | {:to_audit_id, integer()}

  @spec promote(String.t(), String.t(), [mutate_opt()]) ::
          {:ok, map()} | {:error, :pointer_not_found | term()}
  def promote(signature_id, compiled_id, opts \\ [])
      when is_binary(signature_id) and is_binary(compiled_id) and is_list(opts) do
    with {:ok, pointer} <- fetch_pointer(signature_id) do
      before_snapshot = pointer_snapshot(pointer)
      target_artifact = target_artifact(pointer, compiled_id, opts)
      clear_canary = Keyword.get(opts, :clear_canary, true)

      update_attrs =
        %{
          primary_artifact: target_artifact,
          canary_artifact: if(clear_canary, do: nil, else: pointer.canary_artifact),
          canary_percent: if(clear_canary, do: 0, else: pointer.canary_percent),
          rollout_seed: pointer.rollout_seed,
          metadata: pointer.metadata || %{}
        }

      run_pointer_mutation(pointer, update_attrs, "promote", before_snapshot, opts)
    end
  end

  @spec rollback(String.t(), [mutate_opt()]) ::
          {:ok, map()} | {:error, :pointer_not_found | :rollback_unavailable | term()}
  def rollback(signature_id, opts \\ []) when is_binary(signature_id) and is_list(opts) do
    with {:ok, pointer} <- fetch_pointer(signature_id),
         {:ok, source_audit} <- rollback_source_audit(signature_id, opts),
         before_pointer when is_map(before_pointer) <- source_audit.before_pointer do
      before_snapshot = pointer_snapshot(pointer)
      restore_attrs = snapshot_to_update_attrs(before_pointer, pointer)

      run_pointer_mutation(
        pointer,
        restore_attrs,
        "rollback",
        before_snapshot,
        Keyword.put(opts, :target_audit_id, source_audit.id)
      )
    else
      nil -> {:error, :rollback_unavailable}
      {:error, _reason} = error -> error
    end
  end

  @spec list_audits(String.t(), keyword()) :: [PointerAudit.t()]
  def list_audits(signature_id, opts \\ []) when is_binary(signature_id) do
    limit = Keyword.get(opts, :limit, 50)

    query =
      from(audit in PointerAudit,
        where: audit.signature_id == ^signature_id,
        order_by: [desc: audit.inserted_at, desc: audit.id],
        limit: ^limit
      )

    Repo.all(query)
  end

  defp run_pointer_mutation(pointer, update_attrs, action, before_snapshot, opts) do
    actor = Keyword.get(opts, :actor, "system")
    reason = Keyword.get(opts, :reason)
    metadata = normalize_map(Keyword.get(opts, :metadata, %{}))
    target_audit_id = Keyword.get(opts, :target_audit_id)

    multi =
      Multi.new()
      |> Multi.update(
        :pointer,
        PolicyPointer.changeset(
          pointer,
          Map.put(update_attrs, :signature_id, pointer.signature_id)
        )
      )
      |> Multi.insert(:audit, fn %{pointer: updated_pointer} ->
        after_snapshot = pointer_snapshot(updated_pointer)

        PointerAudit.changeset(%PointerAudit{}, %{
          signature_id: pointer.signature_id,
          action: action,
          actor: actor,
          reason: reason,
          metadata: metadata,
          before_pointer: before_snapshot,
          after_pointer: after_snapshot,
          target_audit_id: target_audit_id
        })
      end)

    case Repo.transaction(multi) do
      {:ok, %{pointer: updated_pointer, audit: audit}} ->
        {:ok,
         %{
           signature_id: updated_pointer.signature_id,
           action: action,
           primary_artifact: updated_pointer.primary_artifact,
           canary_artifact: updated_pointer.canary_artifact,
           canary_percent: updated_pointer.canary_percent,
           audit_id: audit.id
         }}

      {:error, _operation, reason, _changes_so_far} ->
        {:error, reason}
    end
  end

  defp fetch_pointer(signature_id) do
    case Repo.get(PolicyPointer, signature_id) do
      nil -> {:error, :pointer_not_found}
      pointer -> {:ok, pointer}
    end
  end

  defp target_artifact(pointer, compiled_id, opts) do
    canary_artifact = normalize_map(pointer.canary_artifact || %{})
    primary_artifact = normalize_map(pointer.primary_artifact || %{})
    canary_compiled_id = canary_artifact["compiled_id"]

    cond do
      canary_compiled_id == compiled_id ->
        canary_artifact

      primary_artifact["compiled_id"] == compiled_id ->
        primary_artifact

      true ->
        %{
          "compiled_id" => compiled_id,
          "strategy_id" => Keyword.get(opts, :strategy_id, "direct.v1")
        }
    end
  end

  defp rollback_source_audit(signature_id, opts) do
    target_audit_id = Keyword.get(opts, :to_audit_id)

    query =
      from(audit in PointerAudit,
        where: audit.signature_id == ^signature_id,
        order_by: [desc: audit.inserted_at, desc: audit.id],
        limit: 1
      )

    query =
      if is_integer(target_audit_id) do
        from(audit in PointerAudit,
          where: audit.signature_id == ^signature_id and audit.id == ^target_audit_id,
          limit: 1
        )
      else
        query
      end

    case Repo.one(query) do
      nil -> {:error, :rollback_unavailable}
      audit -> {:ok, audit}
    end
  end

  defp snapshot_to_update_attrs(snapshot, pointer) do
    snapshot = normalize_map(snapshot)

    %{
      primary_artifact: snapshot["primary_artifact"] || pointer.primary_artifact || %{},
      canary_artifact: snapshot["canary_artifact"],
      canary_percent: snapshot["canary_percent"] || 0,
      rollout_seed: snapshot["rollout_seed"] || pointer.rollout_seed || "default",
      metadata: snapshot["metadata"] || pointer.metadata || %{}
    }
  end

  defp pointer_snapshot(pointer) do
    %{
      "signature_id" => pointer.signature_id,
      "primary_artifact" => normalize_map(pointer.primary_artifact || %{}),
      "canary_artifact" => normalize_map(pointer.canary_artifact),
      "canary_percent" => pointer.canary_percent,
      "rollout_seed" => pointer.rollout_seed,
      "metadata" => normalize_map(pointer.metadata || %{})
    }
  end

  defp normalize_map(nil), do: nil

  defp normalize_map(%{} = map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), normalize_value(value)}
      {key, value} -> {to_string(key), normalize_value(value)}
    end)
  end

  defp normalize_map(_), do: %{}

  defp normalize_value(%{} = value), do: normalize_map(value)
  defp normalize_value(list) when is_list(list), do: Enum.map(list, &normalize_value/1)
  defp normalize_value(value), do: value
end
