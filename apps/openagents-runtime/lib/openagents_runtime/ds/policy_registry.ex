defmodule OpenAgentsRuntime.DS.PolicyRegistry do
  @moduledoc """
  Active artifact pointer lookup and canary selection for DS signatures.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.DS.PolicyPointer
  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.Repo

  @type artifact_descriptor :: %{
          required(String.t()) => term()
        }

  @type pointer_opt ::
          {:run_id, String.t()}
          | {:thread_id, String.t()}
          | {:user_id, String.t() | integer()}
          | {:autopilot_id, String.t()}
          | {:canary_key, String.t()}

  @spec upsert_pointer(String.t(), map()) ::
          {:ok, PolicyPointer.t()} | {:error, Ecto.Changeset.t()}
  def upsert_pointer(signature_id, attrs) when is_binary(signature_id) and is_map(attrs) do
    params =
      %{
        signature_id: signature_id,
        primary_artifact: attrs[:primary_artifact] || attrs["primary_artifact"],
        canary_artifact: attrs[:canary_artifact] || attrs["canary_artifact"],
        canary_percent: attrs[:canary_percent] || attrs["canary_percent"] || 0,
        rollout_seed: attrs[:rollout_seed] || attrs["rollout_seed"] || "default",
        metadata: attrs[:metadata] || attrs["metadata"] || %{}
      }

    changeset = PolicyPointer.changeset(%PolicyPointer{}, params)
    now = DateTime.utc_now()

    Repo.insert(
      changeset,
      conflict_target: [:signature_id],
      on_conflict: [
        set: [
          primary_artifact: Ecto.Changeset.get_field(changeset, :primary_artifact),
          canary_artifact: Ecto.Changeset.get_field(changeset, :canary_artifact),
          canary_percent: Ecto.Changeset.get_field(changeset, :canary_percent),
          rollout_seed: Ecto.Changeset.get_field(changeset, :rollout_seed),
          metadata: Ecto.Changeset.get_field(changeset, :metadata),
          updated_at: now
        ]
      ]
    )
  end

  @spec fetch_pointer(String.t()) :: PolicyPointer.t() | nil
  def fetch_pointer(signature_id) when is_binary(signature_id) do
    Repo.get(PolicyPointer, signature_id)
  end

  @spec clear_canary(String.t()) :: {:ok, PolicyPointer.t()} | {:error, term()}
  def clear_canary(signature_id) when is_binary(signature_id) do
    case fetch_pointer(signature_id) do
      nil ->
        {:error, :pointer_not_found}

      pointer ->
        pointer
        |> PolicyPointer.changeset(%{canary_percent: 0, canary_artifact: nil})
        |> Repo.update()
    end
  end

  @spec active_artifact(String.t(), [pointer_opt()]) :: {:ok, nil | artifact_descriptor()}
  def active_artifact(signature_id, opts \\ []) when is_binary(signature_id) and is_list(opts) do
    case fetch_pointer(signature_id) do
      nil ->
        {:ok, nil}

      %PolicyPointer{} = pointer ->
        canary_key = resolve_canary_key(opts)
        bucket = rollout_bucket(pointer.signature_id, pointer.rollout_seed, canary_key)
        canary_enabled? = pointer.canary_percent > 0 and is_map(pointer.canary_artifact)
        use_canary? = canary_enabled? and bucket < pointer.canary_percent
        selected = if(use_canary?, do: pointer.canary_artifact, else: pointer.primary_artifact)

        artifact =
          selected
          |> normalize_artifact()
          |> Map.merge(%{
            "signature_id" => pointer.signature_id,
            "variant" => if(use_canary?, do: "canary", else: "primary"),
            "canary_percent" => pointer.canary_percent,
            "rollout_bucket" => bucket,
            "rollout_seed" => pointer.rollout_seed,
            "pointer_updated_at" => DateTime.to_iso8601(pointer.updated_at)
          })

        {:ok, artifact}
    end
  end

  @spec selection_preview(String.t(), [pointer_opt()], non_neg_integer()) :: [map()]
  def selection_preview(signature_id, opts \\ [], sample_size \\ 10)
      when is_binary(signature_id) and is_integer(sample_size) and sample_size >= 0 do
    for index <- 1..sample_size do
      key = "#{resolve_canary_key(opts)}:#{index}"
      {:ok, artifact} = active_artifact(signature_id, Keyword.put(opts, :canary_key, key))

      %{
        "sample_key" => key,
        "variant" => artifact && artifact["variant"],
        "compiled_id" => artifact && artifact["compiled_id"],
        "rollout_bucket" => artifact && artifact["rollout_bucket"]
      }
    end
  end

  @spec pointer_count() :: non_neg_integer()
  def pointer_count do
    from(pointer in PolicyPointer, select: count(pointer.signature_id))
    |> Repo.one()
    |> Kernel.||(0)
  end

  defp resolve_canary_key(opts) do
    Keyword.get(opts, :canary_key) ||
      Keyword.get(opts, :run_id) ||
      Keyword.get(opts, :thread_id) ||
      Keyword.get(opts, :autopilot_id) ||
      opts |> Keyword.get(:user_id) |> normalize_user_id() ||
      "default"
  end

  defp normalize_user_id(user_id) when is_integer(user_id), do: Integer.to_string(user_id)
  defp normalize_user_id(user_id) when is_binary(user_id), do: user_id
  defp normalize_user_id(_), do: nil

  defp rollout_bucket(signature_id, rollout_seed, canary_key) do
    "#{signature_id}|#{rollout_seed}|#{canary_key}"
    |> Receipts.stable_hash()
    |> String.slice(0, 8)
    |> Integer.parse(16)
    |> case do
      {value, _} -> rem(value, 100)
      :error -> 0
    end
  end

  defp normalize_artifact(%{} = artifact) do
    Map.new(artifact, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
    |> Map.put_new("strategy_id", "direct.v1")
  end
end
