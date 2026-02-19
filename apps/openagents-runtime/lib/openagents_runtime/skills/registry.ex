defmodule OpenAgentsRuntime.Skills.Registry do
  @moduledoc """
  JSON Tool/Skill Registry with Agent Skills bundle compilation.
  """

  import Ecto.Query

  alias Ecto.Multi
  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Skills.Builtins
  alias OpenAgentsRuntime.Skills.Compatibility
  alias OpenAgentsRuntime.Skills.Compiler
  alias OpenAgentsRuntime.Skills.SkillRelease
  alias OpenAgentsRuntime.Skills.SkillSpec
  alias OpenAgentsRuntime.Skills.SpecValidator
  alias OpenAgentsRuntime.Skills.ToolSpec

  @tool_conflict_fields [
    :tool_pack,
    :name,
    :description,
    :execution_kind,
    :input_schema,
    :output_schema,
    :integration_manifest,
    :auth_requirements,
    :safety_policy,
    :commercial,
    :metadata,
    :submitted_by,
    :state,
    :content_hash,
    :updated_at
  ]

  @skill_conflict_fields [
    :name,
    :description,
    :license,
    :compatibility,
    :instructions_markdown,
    :allowed_tools,
    :scripts,
    :references,
    :assets,
    :commercial,
    :metadata,
    :submitted_by,
    :state,
    :content_hash,
    :updated_at
  ]

  @published_states ~w(validated published)

  @type registry_error ::
          {:invalid_tool_spec, [map()]}
          | {:invalid_skill_spec, [map()]}
          | {:skill_not_found, String.t(), pos_integer()}
          | {:tool_not_found, String.t(), pos_integer() | nil}
          | {:manifest_not_found, String.t(), map()}
          | {:incompatible_bundle, [map()]}
          | Ecto.Changeset.t()

  @spec upsert_tool_spec(map(), keyword()) :: {:ok, ToolSpec.t()} | {:error, registry_error()}
  def upsert_tool_spec(attrs, opts \\ []) when is_map(attrs) do
    state = Keyword.get(opts, :state, "validated")

    with {:ok, normalized} <- SpecValidator.validate_tool_spec(attrs),
         {:ok, state} <- normalize_state(state, ToolSpec.states()) do
      payload =
        normalized
        |> Map.put("state", state)
        |> Map.put("content_hash", content_hash(normalized, state))

      changeset = ToolSpec.changeset(%ToolSpec{}, payload)

      case Repo.insert(changeset,
             on_conflict: {:replace, @tool_conflict_fields},
             conflict_target: [:tool_id, :version],
             returning: true
           ) do
        {:ok, tool_spec} -> {:ok, tool_spec}
        {:error, %Ecto.Changeset{} = changeset} -> {:error, changeset}
      end
    else
      {:error, errors} when is_list(errors) -> {:error, {:invalid_tool_spec, errors}}
    end
  end

  @spec upsert_skill_spec(map(), keyword()) :: {:ok, SkillSpec.t()} | {:error, registry_error()}
  def upsert_skill_spec(attrs, opts \\ []) when is_map(attrs) do
    state = Keyword.get(opts, :state, "validated")

    with {:ok, normalized} <- SpecValidator.validate_skill_spec(attrs),
         {:ok, state} <- normalize_state(state, SkillSpec.states()) do
      payload =
        normalized
        |> Map.put("state", state)
        |> Map.put("content_hash", content_hash(normalized, state))

      changeset = SkillSpec.changeset(%SkillSpec{}, payload)

      case Repo.insert(changeset,
             on_conflict: {:replace, @skill_conflict_fields},
             conflict_target: [:skill_id, :version],
             returning: true
           ) do
        {:ok, skill_spec} -> {:ok, skill_spec}
        {:error, %Ecto.Changeset{} = changeset} -> {:error, changeset}
      end
    else
      {:error, errors} when is_list(errors) -> {:error, {:invalid_skill_spec, errors}}
    end
  end

  @spec publish_skill(String.t(), pos_integer(), keyword()) ::
          {:ok, SkillRelease.t()} | {:error, registry_error()}
  def publish_skill(skill_id, version, opts \\ [])
      when is_binary(skill_id) and is_integer(version) and version > 0 do
    with {:ok, source} <- fetch_skill_source(skill_id, version),
         {:ok, tool_specs} <- resolve_allowed_tools(source.skill["allowed_tools"] || []),
         bundle <- Compiler.compile(source.skill, tool_specs),
         {:ok, compatibility_report} <- Compatibility.validate_bundle(bundle) do
      now = Keyword.get(opts, :now, DateTime.utc_now())
      bundle_hash = Receipts.stable_hash(bundle)

      release_id =
        "skillrel_" <>
          String.slice(
            Receipts.stable_hash(%{
              skill_id: skill_id,
              version: version,
              bundle_hash: bundle_hash
            }),
            0,
            24
          )

      multi =
        Multi.new()
        |> Multi.insert(
          :release,
          SkillRelease.changeset(%SkillRelease{}, %{
            release_id: release_id,
            skill_id: skill_id,
            version: version,
            bundle: bundle,
            bundle_hash: bundle_hash,
            compatibility_report: compatibility_report,
            published_at: now,
            metadata: %{
              source: source.source,
              published_by: Keyword.get(opts, :published_by, "runtime")
            }
          }),
          on_conflict: [
            set: [
              bundle: bundle,
              bundle_hash: bundle_hash,
              compatibility_report: compatibility_report,
              published_at: now,
              metadata: %{
                source: source.source,
                published_by: Keyword.get(opts, :published_by, "runtime")
              },
              updated_at: now
            ]
          ],
          conflict_target: [:skill_id, :version]
        )
        |> maybe_mark_skill_published(source)

      case Repo.transaction(multi) do
        {:ok, %{release: release}} -> {:ok, release}
        {:error, :release, %Ecto.Changeset{} = changeset, _changes} -> {:error, changeset}
        {:error, _step, reason, _changes} -> {:error, reason}
      end
    else
      {:error, {:skill_not_found, _, _} = error} -> {:error, error}
      {:error, {:tool_not_found, _, _} = error} -> {:error, error}
      {:error, errors} when is_list(errors) -> {:error, {:incompatible_bundle, errors}}
    end
  end

  @spec get_skill_release(String.t(), pos_integer()) :: SkillRelease.t() | nil
  def get_skill_release(skill_id, version) when is_binary(skill_id) and is_integer(version) do
    query =
      from(release in SkillRelease,
        where: release.skill_id == ^skill_id and release.version == ^version,
        limit: 1
      )

    Repo.one(query)
  end

  @spec list_tool_specs() :: [map()]
  def list_tool_specs do
    db_specs =
      ToolSpec
      |> order_by([tool_spec], asc: tool_spec.tool_id, asc: tool_spec.version)
      |> Repo.all()
      |> Enum.map(&tool_struct_to_map/1)

    merge_builtin_tools(db_specs)
  end

  @spec list_skill_specs() :: [map()]
  def list_skill_specs do
    db_specs =
      SkillSpec
      |> order_by([skill], asc: skill.skill_id, asc: skill.version)
      |> Repo.all()
      |> Enum.map(&skill_struct_to_map/1)

    merge_builtin_skills(db_specs)
  end

  @spec resolve_tool_manifest(String.t(), map()) :: {:ok, map()} | {:error, registry_error()}
  def resolve_tool_manifest(tool_pack, manifest_ref)
      when is_binary(tool_pack) and is_map(manifest_ref) do
    manifest_ref = stringify_keys(manifest_ref)

    tool_id = normalize_string(manifest_ref["tool_id"])
    integration_id = normalize_string(manifest_ref["integration_id"])
    version = normalize_optional_integer(manifest_ref["version"])

    with {:ok, tool_spec} <- find_manifest_source(tool_pack, tool_id, integration_id, version),
         {:ok, manifest} <- extract_manifest(tool_pack, tool_spec) do
      {:ok, manifest}
    else
      {:error, {:manifest_not_found, _tool_pack, _ref} = error} -> {:error, error}
      {:error, {:tool_not_found, _tool_id, _version} = error} -> {:error, error}
      {:error, reason} -> {:error, reason}
    end
  end

  def resolve_tool_manifest(tool_pack, _manifest_ref) do
    {:error, {:manifest_not_found, tool_pack, %{}}}
  end

  @spec get_tool_spec(String.t(), pos_integer() | nil) :: map() | nil
  def get_tool_spec(tool_id, version \\ nil)

  def get_tool_spec(tool_id, version) when is_binary(tool_id) do
    case db_tool_spec(tool_id, version) do
      nil -> Builtins.find_tool_spec(tool_id, version)
      %ToolSpec{} = spec -> tool_struct_to_map(spec)
    end
  end

  @spec get_skill_spec(String.t(), pos_integer() | nil) :: map() | nil
  def get_skill_spec(skill_id, version \\ nil)

  def get_skill_spec(skill_id, version) when is_binary(skill_id) do
    case db_skill_spec(skill_id, version) do
      nil -> Builtins.find_skill_spec(skill_id, version)
      %SkillSpec{} = spec -> skill_struct_to_map(spec)
    end
  end

  defp maybe_mark_skill_published(multi, %{source: :db, skill_struct: %SkillSpec{} = skill_struct}) do
    now = DateTime.utc_now() |> DateTime.truncate(:second)

    query =
      from(skill in SkillSpec,
        where: skill.skill_id == ^skill_struct.skill_id and skill.version == ^skill_struct.version
      )

    Multi.update_all(multi, :skill, query, set: [state: "published", updated_at: now])
  end

  defp maybe_mark_skill_published(multi, _source), do: multi

  defp find_manifest_source(tool_pack, tool_id, integration_id, version) do
    cond do
      is_binary(tool_id) ->
        case get_tool_spec(tool_id, version) do
          nil -> {:error, {:tool_not_found, tool_id, version}}
          tool_spec -> {:ok, tool_spec}
        end

      is_binary(integration_id) ->
        case find_tool_by_integration(tool_pack, integration_id, version) do
          nil -> {:error, {:manifest_not_found, tool_pack, %{"integration_id" => integration_id}}}
          tool_spec -> {:ok, tool_spec}
        end

      true ->
        {:error,
         {:manifest_not_found, tool_pack, %{"reason" => "missing tool_id/integration_id"}}}
    end
  end

  defp extract_manifest(tool_pack, tool_spec) do
    tool_spec = stringify_keys(tool_spec)
    manifest = stringify_keys(tool_spec["integration_manifest"])

    cond do
      not is_map(manifest) ->
        {:error, {:manifest_not_found, tool_pack, %{"reason" => "integration_manifest missing"}}}

      tool_spec["tool_pack"] != tool_pack ->
        {:error, {:manifest_not_found, tool_pack, %{"reason" => "tool_pack mismatch"}}}

      manifest["tool_pack"] != tool_pack ->
        {:error, {:manifest_not_found, tool_pack, %{"reason" => "manifest tool_pack mismatch"}}}

      true ->
        {:ok, manifest}
    end
  end

  defp find_tool_by_integration(tool_pack, integration_id, version) do
    query =
      ToolSpec
      |> where([tool_spec], tool_spec.tool_pack == ^tool_pack)
      |> where(
        [tool_spec],
        fragment("?->>'integration_id'", tool_spec.integration_manifest) == ^integration_id
      )
      |> where([tool_spec], tool_spec.state in ^@published_states)
      |> maybe_filter_tool_version(version)
      |> order_by([tool_spec], desc: tool_spec.version)
      |> limit(1)

    case Repo.one(query) do
      nil -> Builtins.find_tool_spec_by_integration(tool_pack, integration_id, version)
      %ToolSpec{} = spec -> tool_struct_to_map(spec)
    end
  end

  defp maybe_filter_tool_version(query, nil), do: query

  defp maybe_filter_tool_version(query, version) when is_integer(version) do
    where(query, [tool_spec], tool_spec.version == ^version)
  end

  defp resolve_allowed_tools(allowed_tools) when is_list(allowed_tools) do
    Enum.reduce_while(allowed_tools, {:ok, []}, fn tool_ref, {:ok, acc} ->
      tool_ref = stringify_keys(tool_ref)
      tool_id = normalize_string(tool_ref["tool_id"])
      version = normalize_optional_integer(tool_ref["version"])

      case tool_id do
        nil ->
          {:halt, {:error, {:tool_not_found, "unknown", version}}}

        _tool_id ->
          case get_tool_spec(tool_id, version) do
            nil -> {:halt, {:error, {:tool_not_found, tool_id, version}}}
            tool_spec -> {:cont, {:ok, [tool_spec | acc]}}
          end
      end
    end)
    |> case do
      {:ok, resolved} -> {:ok, Enum.reverse(resolved)}
      {:error, _reason} = error -> error
    end
  end

  defp resolve_allowed_tools(_), do: {:ok, []}

  defp fetch_skill_source(skill_id, version) do
    case db_skill_spec(skill_id, version) do
      %SkillSpec{} = skill_spec ->
        {:ok, %{source: :db, skill: skill_struct_to_map(skill_spec), skill_struct: skill_spec}}

      nil ->
        case Builtins.find_skill_spec(skill_id, version) do
          nil -> {:error, {:skill_not_found, skill_id, version}}
          skill_spec -> {:ok, %{source: :builtin, skill: skill_spec, skill_struct: nil}}
        end
    end
  end

  defp db_skill_spec(skill_id, version) do
    query =
      SkillSpec
      |> where([skill], skill.skill_id == ^skill_id)
      |> where([skill], skill.version == ^version)
      |> limit(1)

    Repo.one(query)
  end

  defp db_tool_spec(tool_id, version) do
    base_query =
      ToolSpec
      |> where([tool_spec], tool_spec.tool_id == ^tool_id)
      |> where([tool_spec], tool_spec.state in ^@published_states)

    query =
      case version do
        nil -> base_query |> order_by([tool_spec], desc: tool_spec.version) |> limit(1)
        version -> base_query |> where([tool_spec], tool_spec.version == ^version) |> limit(1)
      end

    Repo.one(query)
  end

  defp merge_builtin_tools(db_specs) do
    Enum.reduce(Builtins.tool_specs(), db_specs, fn builtin, acc ->
      exists? =
        Enum.any?(
          acc,
          &(&1["tool_id"] == builtin["tool_id"] and &1["version"] == builtin["version"])
        )

      if exists?, do: acc, else: acc ++ [builtin]
    end)
  end

  defp merge_builtin_skills(db_specs) do
    Enum.reduce(Builtins.skill_specs(), db_specs, fn builtin, acc ->
      exists? =
        Enum.any?(
          acc,
          &(&1["skill_id"] == builtin["skill_id"] and &1["version"] == builtin["version"])
        )

      if exists?, do: acc, else: acc ++ [builtin]
    end)
  end

  defp tool_struct_to_map(%ToolSpec{} = spec) do
    %{
      "tool_id" => spec.tool_id,
      "version" => spec.version,
      "tool_pack" => spec.tool_pack,
      "name" => spec.name,
      "description" => spec.description,
      "execution_kind" => spec.execution_kind,
      "input_schema" => spec.input_schema || %{},
      "output_schema" => spec.output_schema || %{},
      "integration_manifest" => spec.integration_manifest || %{},
      "auth_requirements" => spec.auth_requirements || %{},
      "safety_policy" => spec.safety_policy || %{},
      "commercial" => spec.commercial || %{},
      "metadata" => spec.metadata || %{},
      "submitted_by" => spec.submitted_by,
      "state" => spec.state,
      "content_hash" => spec.content_hash
    }
  end

  defp skill_struct_to_map(%SkillSpec{} = spec) do
    %{
      "skill_id" => spec.skill_id,
      "version" => spec.version,
      "name" => spec.name,
      "description" => spec.description,
      "license" => spec.license,
      "compatibility" => spec.compatibility || %{},
      "instructions_markdown" => spec.instructions_markdown,
      "allowed_tools" => spec.allowed_tools || [],
      "scripts" => spec.scripts || [],
      "references" => spec.references || [],
      "assets" => spec.assets || [],
      "commercial" => spec.commercial || %{},
      "metadata" => spec.metadata || %{},
      "submitted_by" => spec.submitted_by,
      "state" => spec.state,
      "content_hash" => spec.content_hash
    }
  end

  defp normalize_state(state, allowed_states) do
    state = normalize_string(state)

    cond do
      is_nil(state) -> {:ok, "validated"}
      state in allowed_states -> {:ok, state}
      true -> {:error, ["invalid state: #{inspect(state)}"]}
    end
  end

  defp content_hash(spec, state) do
    spec
    |> Map.put("state", state)
    |> Receipts.stable_hash()
  end

  defp normalize_optional_integer(value) when is_integer(value) and value > 0, do: value

  defp normalize_optional_integer(value) when is_binary(value) do
    case Integer.parse(value) do
      {parsed, ""} when parsed > 0 -> parsed
      _ -> nil
    end
  end

  defp normalize_optional_integer(_), do: nil

  defp normalize_string(value) when is_binary(value) do
    value
    |> String.trim()
    |> case do
      "" -> nil
      normalized -> normalized
    end
  end

  defp normalize_string(_), do: nil

  defp stringify_keys(nil), do: %{}

  defp stringify_keys(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end
end
