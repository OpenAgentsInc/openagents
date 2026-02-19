defmodule OpenAgentsRuntimeWeb.SkillRegistryController do
  use OpenAgentsRuntimeWeb, :controller

  alias OpenAgentsRuntime.Skills.Registry

  @spec list_tool_specs(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def list_tool_specs(conn, _params) do
    json(conn, %{"data" => Registry.list_tool_specs()})
  end

  @spec upsert_tool_spec(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def upsert_tool_spec(conn, params) do
    with {:ok, user_id} <- require_user_id(conn),
         {:ok, tool_spec} <- require_map(params, "tool_spec"),
         {:ok, result} <-
           Registry.upsert_tool_spec(
             Map.put(tool_spec, "submitted_by", "user:#{user_id}"),
             state: params["state"] || "validated"
           ) do
      conn
      |> put_status(201)
      |> json(%{
        "data" => %{
          "tool_id" => result.tool_id,
          "version" => result.version,
          "state" => result.state
        }
      })
    else
      {:error, :missing_user_id} ->
        error(conn, 400, "invalid_request", "x-oa-user-id header is required")

      {:error, {:missing_field, field}} ->
        error(conn, 400, "invalid_request", "#{field} is required and must be an object")

      {:error, {:invalid_tool_spec, details}} ->
        validation_error(conn, details)

      {:error, %Ecto.Changeset{} = changeset} ->
        validation_error(conn, changeset_errors(changeset))
    end
  end

  @spec list_skill_specs(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def list_skill_specs(conn, _params) do
    json(conn, %{"data" => Registry.list_skill_specs()})
  end

  @spec upsert_skill_spec(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def upsert_skill_spec(conn, params) do
    with {:ok, user_id} <- require_user_id(conn),
         {:ok, skill_spec} <- require_map(params, "skill_spec"),
         {:ok, result} <-
           Registry.upsert_skill_spec(
             Map.put(skill_spec, "submitted_by", "user:#{user_id}"),
             state: params["state"] || "validated"
           ) do
      conn
      |> put_status(201)
      |> json(%{
        "data" => %{
          "skill_id" => result.skill_id,
          "version" => result.version,
          "state" => result.state
        }
      })
    else
      {:error, :missing_user_id} ->
        error(conn, 400, "invalid_request", "x-oa-user-id header is required")

      {:error, {:missing_field, field}} ->
        error(conn, 400, "invalid_request", "#{field} is required and must be an object")

      {:error, {:invalid_skill_spec, details}} ->
        validation_error(conn, details)

      {:error, %Ecto.Changeset{} = changeset} ->
        validation_error(conn, changeset_errors(changeset))
    end
  end

  @spec publish_skill(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def publish_skill(conn, %{"skill_id" => skill_id, "version" => version}) do
    with {:ok, user_id} <- require_user_id(conn),
         {:ok, version} <- parse_positive_integer(version),
         {:ok, release} <-
           Registry.publish_skill(skill_id, version, published_by: "user:#{user_id}") do
      conn
      |> put_status(201)
      |> json(%{
        "data" => %{
          "release_id" => release.release_id,
          "skill_id" => release.skill_id,
          "version" => release.version,
          "bundle_hash" => release.bundle_hash,
          "published_at" => DateTime.to_iso8601(release.published_at)
        }
      })
    else
      {:error, :missing_user_id} ->
        error(conn, 400, "invalid_request", "x-oa-user-id header is required")

      {:error, :invalid_version} ->
        error(conn, 400, "invalid_request", "version must be a positive integer")

      {:error, {:skill_not_found, _skill_id, _version}} ->
        error(conn, 404, "not_found", "skill spec not found")

      {:error, {:tool_not_found, tool_id, version}} ->
        validation_error(conn, [
          %{
            "reason_code" => "skill_registry.tool_not_found",
            "path" => "allowed_tools",
            "message" => "tool not found: #{tool_id}@#{version || "latest"}"
          }
        ])

      {:error, {:incompatible_bundle, details}} ->
        validation_error(conn, details)

      {:error, %Ecto.Changeset{} = changeset} ->
        validation_error(conn, changeset_errors(changeset))
    end
  end

  def publish_skill(conn, _params) do
    error(conn, 400, "invalid_request", "skill_id and version are required")
  end

  @spec show_skill_release(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def show_skill_release(conn, %{"skill_id" => skill_id, "version" => version}) do
    with {:ok, version} <- parse_positive_integer(version) do
      case Registry.get_skill_release(skill_id, version) do
        nil ->
          error(conn, 404, "not_found", "skill release not found")

        release ->
          json(conn, %{
            "data" => %{
              "release_id" => release.release_id,
              "skill_id" => release.skill_id,
              "version" => release.version,
              "bundle" => release.bundle,
              "bundle_hash" => release.bundle_hash,
              "compatibility_report" => release.compatibility_report,
              "published_at" => DateTime.to_iso8601(release.published_at)
            }
          })
      end
    else
      {:error, :invalid_version} ->
        error(conn, 400, "invalid_request", "version must be a positive integer")
    end
  end

  def show_skill_release(conn, _params) do
    error(conn, 400, "invalid_request", "skill_id and version are required")
  end

  defp require_map(params, key) do
    case Map.get(params, key) do
      value when is_map(value) -> {:ok, value}
      _ -> {:error, {:missing_field, key}}
    end
  end

  defp parse_positive_integer(value) when is_integer(value) and value > 0, do: {:ok, value}

  defp parse_positive_integer(value) when is_binary(value) do
    case Integer.parse(value) do
      {parsed, ""} when parsed > 0 -> {:ok, parsed}
      _ -> {:error, :invalid_version}
    end
  end

  defp parse_positive_integer(_), do: {:error, :invalid_version}

  defp require_user_id(conn) do
    case conn |> get_req_header("x-oa-user-id") |> List.first() do
      nil ->
        {:error, :missing_user_id}

      value ->
        case Integer.parse(value) do
          {user_id, ""} when user_id > 0 -> {:ok, user_id}
          _ -> {:error, :missing_user_id}
        end
    end
  end

  defp validation_error(conn, details) do
    conn
    |> put_status(422)
    |> json(%{
      "error" => %{
        "code" => "invalid_request",
        "message" => "skill registry validation failed",
        "details" => details
      }
    })
  end

  defp changeset_errors(%Ecto.Changeset{} = changeset) do
    Ecto.Changeset.traverse_errors(changeset, fn {msg, _opts} -> msg end)
    |> Enum.map(fn {field, messages} ->
      %{
        "reason_code" => "skill_registry.invalid_changeset",
        "path" => to_string(field),
        "message" => Enum.join(messages, ", ")
      }
    end)
  end

  defp error(conn, status, code, message) do
    conn
    |> put_status(status)
    |> json(%{"error" => %{"code" => code, "message" => message}})
  end
end
