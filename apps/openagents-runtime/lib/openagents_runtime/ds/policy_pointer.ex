defmodule OpenAgentsRuntime.DS.PolicyPointer do
  @moduledoc """
  Active/canary artifact pointer record for DS signature execution.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  @primary_key {:signature_id, :string, autogenerate: false}

  schema "ds_artifact_pointers" do
    field :primary_artifact, :map
    field :canary_artifact, :map
    field :canary_percent, :integer, default: 0
    field :rollout_seed, :string, default: "default"
    field :metadata, :map, default: %{}

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(signature_id primary_artifact canary_percent rollout_seed)a

  @type t :: %__MODULE__{
          signature_id: String.t(),
          primary_artifact: map(),
          canary_artifact: map() | nil,
          canary_percent: non_neg_integer(),
          rollout_seed: String.t(),
          metadata: map(),
          inserted_at: DateTime.t(),
          updated_at: DateTime.t()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(pointer, attrs) do
    pointer
    |> cast(attrs, @required_fields ++ [:canary_artifact, :metadata])
    |> validate_required(@required_fields)
    |> validate_length(:signature_id, min: 1, max: 255)
    |> validate_number(:canary_percent, greater_than_or_equal_to: 0, less_than_or_equal_to: 100)
    |> validate_artifact_map(:primary_artifact, required: true)
    |> validate_artifact_map(:canary_artifact, required: false)
    |> validate_canary_requirements()
  end

  defp validate_artifact_map(changeset, field, opts) do
    required? = Keyword.get(opts, :required, false)
    artifact = get_field(changeset, field)

    cond do
      is_nil(artifact) and not required? ->
        changeset

      not is_map(artifact) ->
        add_error(changeset, field, "must be a map artifact descriptor")

      not has_compiled_id?(artifact) ->
        add_error(changeset, field, "must include compiled_id")

      true ->
        artifact =
          artifact
          |> normalize_artifact()
          |> Map.put_new("strategy_id", "direct.v1")

        put_change(changeset, field, artifact)
    end
  end

  defp validate_canary_requirements(changeset) do
    canary_percent = get_field(changeset, :canary_percent) || 0
    canary_artifact = get_field(changeset, :canary_artifact)

    if canary_percent > 0 and is_nil(canary_artifact) do
      add_error(changeset, :canary_artifact, "is required when canary_percent > 0")
    else
      changeset
    end
  end

  defp has_compiled_id?(artifact) do
    compiled_id = artifact[:compiled_id] || artifact["compiled_id"]
    is_binary(compiled_id) and String.trim(compiled_id) != ""
  end

  defp normalize_artifact(artifact) do
    Map.new(artifact, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end
end
