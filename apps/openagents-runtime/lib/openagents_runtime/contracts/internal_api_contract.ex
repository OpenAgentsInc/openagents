defmodule OpenAgentsRuntime.Contracts.InternalAPIContract do
  @moduledoc """
  Checks convergence between implemented `/internal/v1/*` routes and contract artifacts.
  """

  alias OpenAgentsRuntimeWeb.Router
  alias YamlElixir

  @openapi_path Path.expand("../../../docs/openapi-internal-v1.yaml", __DIR__)
  @runtime_contract_path Path.expand("../../../docs/RUNTIME_CONTRACT.md", __DIR__)

  @http_methods ~w(get post put patch delete options head)

  @expected_operations %{
    {"/health", "get"} => %{
      statuses: ~w(200 401),
      required_params: []
    },
    {"/comms/delivery-events", "post"} => %{
      statuses: ~w(200 202 400 401 409),
      required_params: []
    },
    {"/tools/execute", "post"} => %{
      statuses: ~w(200 400 401 403 422),
      required_params: []
    },
    {"/skills/tool-specs", "get"} => %{
      statuses: ~w(200 401),
      required_params: []
    },
    {"/skills/tool-specs", "post"} => %{
      statuses: ~w(201 400 401 422),
      required_params: []
    },
    {"/skills/skill-specs", "get"} => %{
      statuses: ~w(200 401),
      required_params: []
    },
    {"/skills/skill-specs", "post"} => %{
      statuses: ~w(201 400 401 422),
      required_params: []
    },
    {"/skills/skill-specs/{skill_id}/{version}/publish", "post"} => %{
      statuses: ~w(201 400 401 404 422),
      required_params: [
        %{name: "skill_id", location: "path", required: true},
        %{name: "version", location: "path", required: true}
      ]
    },
    {"/skills/releases/{skill_id}/{version}", "get"} => %{
      statuses: ~w(200 400 401 404),
      required_params: [
        %{name: "skill_id", location: "path", required: true},
        %{name: "version", location: "path", required: true}
      ]
    },
    {"/codex/workers", "get"} => %{
      statuses: ~w(200 400 401),
      required_params: [],
      optional_params: [
        %{name: "status", location: "query", required: false},
        %{name: "workspace_ref", location: "query", required: false},
        %{name: "limit", location: "query", required: false}
      ]
    },
    {"/codex/workers", "post"} => %{
      statuses: ~w(200 202 400 401 403),
      required_params: []
    },
    {"/codex/workers/{worker_id}/snapshot", "get"} => %{
      statuses: ~w(200 400 401 403 404),
      required_params: [
        %{name: "worker_id", location: "path", required: true}
      ]
    },
    {"/codex/workers/{worker_id}/requests", "post"} => %{
      statuses: ~w(200 400 401 403 404),
      required_params: [
        %{name: "worker_id", location: "path", required: true}
      ]
    },
    {"/codex/workers/{worker_id}/stream", "get"} => %{
      statuses: ~w(200 400 401 403 404 410),
      required_params: [
        %{name: "worker_id", location: "path", required: true}
      ],
      optional_params: [
        %{name: "cursor", location: "query", required: false},
        %{name: "tail_ms", location: "query", required: false},
        %{name: "Last-Event-ID", location: "header", required: false}
      ]
    },
    {"/codex/workers/{worker_id}/stop", "post"} => %{
      statuses: ~w(200 202 400 401 403 404),
      required_params: [
        %{name: "worker_id", location: "path", required: true}
      ]
    },
    {"/runs/{run_id}/snapshot", "get"} => %{
      statuses: ~w(200 400 401 403 404),
      required_params: [
        %{name: "run_id", location: "path", required: true},
        %{name: "thread_id", location: "query", required: true}
      ]
    },
    {"/runs/{run_id}/frames", "post"} => %{
      statuses: ~w(200 202 400 401 403 404 409),
      required_params: [
        %{name: "run_id", location: "path", required: true}
      ]
    },
    {"/runs/{run_id}/stream", "get"} => %{
      statuses: ~w(200 400 401 403 404 410),
      required_params: [
        %{name: "run_id", location: "path", required: true},
        %{name: "thread_id", location: "query", required: true}
      ],
      optional_params: [
        %{name: "cursor", location: "query", required: false},
        %{name: "tail_ms", location: "query", required: false},
        %{name: "Last-Event-ID", location: "header", required: false}
      ]
    },
    {"/runs/{run_id}/cancel", "post"} => %{
      statuses: ~w(200 202 400 401 403 404 500),
      required_params: [
        %{name: "run_id", location: "path", required: true}
      ]
    }
  }

  @doc """
  Runs all internal contract checks.
  """
  @spec check() :: :ok | {:error, [String.t()]}
  def check do
    {spec, errors} = load_openapi(@openapi_path)

    errors =
      []
      |> maybe_add_artifact_error(@openapi_path)
      |> maybe_add_artifact_error(@runtime_contract_path)
      |> Kernel.++(errors)

    errors =
      case spec do
        nil ->
          errors

        parsed_spec ->
          errors
          |> Kernel.++(validate_openapi_shape(parsed_spec))
          |> Kernel.++(validate_route_convergence(parsed_spec))
          |> Kernel.++(validate_expected_operation_contracts(parsed_spec))
      end

    errors =
      errors
      |> Kernel.++(validate_runtime_contract_markdown(@runtime_contract_path))
      |> Enum.uniq()

    if errors == [], do: :ok, else: {:error, errors}
  end

  @spec openapi_path() :: String.t()
  def openapi_path, do: @openapi_path

  @spec runtime_contract_path() :: String.t()
  def runtime_contract_path, do: @runtime_contract_path

  defp maybe_add_artifact_error(errors, path) do
    if File.exists?(path) do
      errors
    else
      ["missing artifact: #{path}" | errors]
    end
  end

  defp load_openapi(path) do
    if File.exists?(path) do
      case YamlElixir.read_from_file(path) do
        {:ok, spec} when is_map(spec) ->
          {spec, []}

        {:ok, _other} ->
          {nil, ["openapi artifact is not a map: #{path}"]}

        {:error, reason} ->
          {nil, ["openapi artifact parse error: #{path} (#{inspect(reason)})"]}
      end
    else
      {nil, []}
    end
  end

  defp validate_openapi_shape(spec) do
    []
    |> maybe_push(is_binary(spec["openapi"]), "openapi version missing or invalid")
    |> maybe_push(is_map(spec["info"]), "openapi info section missing")
    |> maybe_push(is_map(spec["paths"]), "openapi paths section missing")
  end

  defp validate_route_convergence(spec) do
    spec_operations =
      spec["paths"]
      |> Enum.flat_map(fn {path, operations} ->
        operations
        |> Enum.filter(fn {operation, _} -> operation in @http_methods end)
        |> Enum.map(fn {operation, _} -> {path, operation} end)
      end)
      |> MapSet.new()

    router_operations =
      Router.__routes__()
      |> Enum.filter(&String.starts_with?(&1.path, "/internal/v1"))
      |> Enum.map(fn route ->
        path =
          route.path
          |> String.replace_prefix("/internal/v1", "")
          |> String.replace(~r/:([a-zA-Z0-9_]+)/, "{\\1}")

        {path, route.verb |> to_string() |> String.downcase()}
      end)
      |> MapSet.new()

    missing_in_spec = MapSet.difference(router_operations, spec_operations) |> MapSet.to_list()
    extra_in_spec = MapSet.difference(spec_operations, router_operations) |> MapSet.to_list()

    []
    |> append_operation_errors("missing route operation in OpenAPI", missing_in_spec)
    |> append_operation_errors("OpenAPI operation not implemented in router", extra_in_spec)
  end

  defp append_operation_errors(errors, _label, []), do: errors

  defp append_operation_errors(errors, label, operations) do
    Enum.reduce(operations, errors, fn {path, method}, acc ->
      ["#{label}: #{String.upcase(method)} #{path}" | acc]
    end)
  end

  defp validate_expected_operation_contracts(spec) do
    Enum.reduce(@expected_operations, [], fn {{path, method}, expectation}, errors ->
      operation = get_in(spec, ["paths", path, method])

      errors =
        if is_map(operation) do
          errors
        else
          ["expected operation missing: #{String.upcase(method)} #{path}" | errors]
        end

      if is_map(operation) do
        parameters = collect_operation_parameters(spec, path, operation)
        statuses = operation["responses"] |> Kernel.||(%{}) |> Map.keys() |> MapSet.new()

        errors
        |> validate_required_parameters(
          path,
          method,
          expectation[:required_params] || [],
          parameters
        )
        |> validate_optional_parameters(
          path,
          method,
          expectation[:optional_params] || [],
          parameters
        )
        |> validate_statuses(path, method, expectation[:statuses], statuses)
      else
        errors
      end
    end)
  end

  defp collect_operation_parameters(spec, path, operation) do
    path_parameters = get_in(spec, ["paths", path, "parameters"]) || []
    operation_parameters = operation["parameters"] || []

    (path_parameters ++ operation_parameters)
    |> Enum.map(&resolve_parameter(spec, &1))
    |> Enum.filter(&is_map/1)
  end

  defp resolve_parameter(spec, %{"$ref" => ref}) when is_binary(ref) do
    case String.split(ref, "/", trim: true) do
      ["#", "components", "parameters", key] -> get_in(spec, ["components", "parameters", key])
      _ -> nil
    end
  end

  defp resolve_parameter(_spec, parameter), do: parameter

  defp validate_required_parameters(errors, path, method, expected, parameters) do
    Enum.reduce(expected, errors, fn param, acc ->
      if has_parameter?(parameters, param) do
        acc
      else
        [
          "missing required parameter in OpenAPI for #{String.upcase(method)} #{path}: #{param.name} (#{param.location})"
          | acc
        ]
      end
    end)
  end

  defp validate_optional_parameters(errors, path, method, expected, parameters) do
    Enum.reduce(expected, errors, fn param, acc ->
      if has_parameter?(parameters, param) do
        acc
      else
        [
          "missing optional parameter in OpenAPI for #{String.upcase(method)} #{path}: #{param.name} (#{param.location})"
          | acc
        ]
      end
    end)
  end

  defp has_parameter?(parameters, %{name: name, location: location, required: required}) do
    Enum.any?(parameters, fn parameter ->
      parameter["name"] == name and parameter["in"] == location and
        parameter["required"] == required
    end)
  end

  defp validate_statuses(errors, path, method, expected_statuses, actual_statuses) do
    expected_set = MapSet.new(expected_statuses)
    missing = MapSet.difference(expected_set, actual_statuses) |> MapSet.to_list()
    extra = MapSet.difference(actual_statuses, expected_set) |> MapSet.to_list()

    errors =
      Enum.reduce(missing, errors, fn status, acc ->
        [
          "missing response status in OpenAPI for #{String.upcase(method)} #{path}: #{status}"
          | acc
        ]
      end)

    Enum.reduce(extra, errors, fn status, acc ->
      [
        "unexpected response status in OpenAPI for #{String.upcase(method)} #{path}: #{status}"
        | acc
      ]
    end)
  end

  defp validate_runtime_contract_markdown(path) do
    with true <- File.exists?(path),
         {:ok, body} <- File.read(path) do
      []
      |> maybe_push(
        String.contains?(body, "POST /internal/v1/comms/delivery-events"),
        "RUNTIME_CONTRACT.md missing comms delivery-events endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "POST /internal/v1/tools/execute"),
        "RUNTIME_CONTRACT.md missing tools execute endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "GET /internal/v1/skills/tool-specs"),
        "RUNTIME_CONTRACT.md missing skill registry tool-specs endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "POST /internal/v1/skills/skill-specs"),
        "RUNTIME_CONTRACT.md missing skill registry skill-specs endpoint section"
      )
      |> maybe_push(
        String.contains?(
          body,
          "POST /internal/v1/skills/skill-specs/{skill_id}/{version}/publish"
        ),
        "RUNTIME_CONTRACT.md missing skill publish endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "GET /internal/v1/skills/releases/{skill_id}/{version}"),
        "RUNTIME_CONTRACT.md missing skill release endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "GET /internal/v1/codex/workers"),
        "RUNTIME_CONTRACT.md missing codex workers list endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "POST /internal/v1/codex/workers"),
        "RUNTIME_CONTRACT.md missing codex workers create endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "workspace_ref"),
        "RUNTIME_CONTRACT.md missing codex workers list query semantics"
      )
      |> maybe_push(
        String.contains?(body, "POST /internal/v1/codex/workers/{worker_id}/requests"),
        "RUNTIME_CONTRACT.md missing codex worker request endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "GET /internal/v1/codex/workers/{worker_id}/stream"),
        "RUNTIME_CONTRACT.md missing codex worker stream endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "POST /internal/v1/codex/workers/{worker_id}/stop"),
        "RUNTIME_CONTRACT.md missing codex worker stop endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "GET /internal/v1/runs/{run_id}/snapshot"),
        "RUNTIME_CONTRACT.md missing snapshot endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "POST /internal/v1/runs/{run_id}/frames"),
        "RUNTIME_CONTRACT.md missing append frame endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "GET /internal/v1/runs/{run_id}/stream"),
        "RUNTIME_CONTRACT.md missing stream endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "POST /internal/v1/runs/{run_id}/cancel"),
        "RUNTIME_CONTRACT.md missing cancel endpoint section"
      )
      |> maybe_push(
        String.contains?(body, "tail_ms"),
        "RUNTIME_CONTRACT.md missing tail_ms semantics"
      )
    else
      false -> []
      {:error, reason} -> ["RUNTIME_CONTRACT.md cannot be read: #{inspect(reason)}"]
    end
  end

  defp maybe_push(errors, true, _message), do: errors
  defp maybe_push(errors, false, message), do: [message | errors]
end
