defmodule OpenAgentsRuntime.DS.Signatures.Catalog do
  @moduledoc """
  DS signature catalog with deterministic IDs and hash utilities.

  The catalog is the compatibility anchor for compiled artifacts and receipts.
  """

  @catalog_version 1

  @signatures [
    %{
      namespace: "@openagents/autopilot/blueprint",
      name: "SelectTool",
      version: 1,
      input_schema: %{
        "messages" => [%{"role" => "string", "content" => "string"}],
        "tools" => [%{"name" => "string", "description" => "string"}],
        "context" => %{"memory" => "map"}
      },
      output_schema: %{
        "tool_name" => "string",
        "arguments" => "map",
        "confidence" => "number"
      },
      prompt_template: "Choose the best tool for the current objective.",
      program_template: "direct.v1"
    },
    %{
      namespace: "@openagents/autopilot/canary",
      name: "RecapThread",
      version: 1,
      input_schema: %{
        "thread_id" => "string",
        "messages" => [%{"role" => "string", "content" => "string"}]
      },
      output_schema: %{
        "summary" => "string",
        "action_items" => ["string"]
      },
      prompt_template: "Summarize the thread and extract concrete action items.",
      program_template: "direct.v1"
    },
    %{
      namespace: "@openagents/autopilot/rlm",
      name: "SummarizeThread",
      version: 1,
      input_schema: %{
        "timeline_window" => "map",
        "tool_replay" => "map"
      },
      output_schema: %{
        "summary" => "string",
        "citations" => ["string"],
        "confidence" => "number"
      },
      prompt_template: "Produce a bounded, citation-focused thread summary.",
      program_template: "rlm_lite.v1"
    },
    %{
      namespace: "@openagents/autopilot/workflow",
      name: "StructuredTask",
      version: 1,
      input_schema: %{
        "task" => %{
          "id" => "string",
          "objective" => "string"
        },
        "context" => "map",
        "tools" => [%{"name" => "string", "description" => "string"}]
      },
      output_schema: %{
        "status" => "string",
        "result" => "map",
        "next_actions" => ["string"],
        "confidence" => "number"
      },
      prompt_template:
        "Execute a structured workflow task and return typed outputs with explicit next actions.",
      program_template: "direct.v1"
    },
    %{
      namespace: "@openagents/autopilot/workflow",
      name: "TimelineMapItem",
      version: 1,
      input_schema: %{
        "query" => "string",
        "item" => "map",
        "item_index" => "number"
      },
      output_schema: %{
        "item_index" => "number",
        "summary" => "string",
        "signals" => ["string"],
        "confidence" => "number"
      },
      prompt_template: "Map one timeline item into a compact typed summary.",
      program_template: "direct.v1"
    },
    %{
      namespace: "@openagents/autopilot/workflow",
      name: "TimelineMapReduce",
      version: 1,
      input_schema: %{
        "query" => "string",
        "mapped_items" => ["map"]
      },
      output_schema: %{
        "summary" => "string",
        "highlights" => ["string"],
        "item_count" => "number",
        "confidence" => "number"
      },
      prompt_template:
        "Reduce mapped timeline summaries into one bounded synthesis with highlights and confidence.",
      program_template: "rlm_lite.v1"
    }
  ]

  @type signature_definition :: %{
          required(:signature_id) => String.t(),
          required(:namespace) => String.t(),
          required(:name) => String.t(),
          required(:version) => pos_integer(),
          required(:input_schema) => map(),
          required(:output_schema) => map(),
          required(:prompt_template) => String.t(),
          required(:program_template) => String.t()
        }

  @type hash_key :: :schema_hash | :prompt_hash | :program_hash

  @spec catalog_version() :: pos_integer()
  def catalog_version, do: @catalog_version

  @spec signatures() :: [signature_definition()]
  def signatures do
    Enum.map(@signatures, &hydrate_signature/1)
  end

  @spec signature_ids() :: [String.t()]
  def signature_ids do
    signatures() |> Enum.map(& &1.signature_id)
  end

  @spec fetch(String.t()) :: {:ok, signature_definition()} | {:error, :not_found}
  def fetch(signature_id) when is_binary(signature_id) do
    case Enum.find(signatures(), &(&1.signature_id == signature_id)) do
      nil -> {:error, :not_found}
      signature -> {:ok, signature}
    end
  end

  @spec stable_signature_id(String.t(), String.t(), pos_integer()) :: String.t()
  def stable_signature_id(namespace, name, version)
      when is_binary(namespace) and is_binary(name) and is_integer(version) and version > 0 do
    "#{namespace}/#{name}.v#{version}"
  end

  @spec hashes(String.t() | signature_definition()) ::
          {:ok, %{schema_hash: String.t(), prompt_hash: String.t(), program_hash: String.t()}}
          | {:error, :not_found}
  def hashes(signature_id) when is_binary(signature_id) do
    with {:ok, signature} <- fetch(signature_id) do
      {:ok, hashes_for(signature)}
    end
  end

  def hashes(%{} = signature) do
    {:ok, hashes_for(signature)}
  end

  @spec fingerprint(String.t()) :: {:ok, map()} | {:error, :not_found}
  def fingerprint(signature_id) when is_binary(signature_id) do
    with {:ok, signature} <- fetch(signature_id),
         {:ok, hashes} <- hashes(signature) do
      {:ok,
       %{
         signature_id: signature.signature_id,
         schema_hash: hashes.schema_hash,
         prompt_hash: hashes.prompt_hash,
         program_hash: hashes.program_hash,
         catalog_version: @catalog_version
       }}
    end
  end

  @spec artifact_compatible?(String.t(), map()) :: boolean()
  def artifact_compatible?(signature_id, artifact)
      when is_binary(signature_id) and is_map(artifact) do
    case validate_artifact(signature_id, artifact) do
      :ok -> true
      {:error, _reason} -> false
    end
  end

  @spec validate_artifact(String.t(), map()) ::
          :ok
          | {:error, :signature_not_found}
          | {:error, {:missing_hash, hash_key()}}
          | {:error, {:hash_mismatch, hash_key()}}
  def validate_artifact(signature_id, artifact)
      when is_binary(signature_id) and is_map(artifact) do
    with {:ok, expected} <- hashes(signature_id) do
      Enum.reduce_while([:schema_hash, :prompt_hash, :program_hash], :ok, fn key, _acc ->
        expected_value = Map.fetch!(expected, key)
        provided_value = Map.get(artifact, key) || Map.get(artifact, Atom.to_string(key))

        case provided_value do
          nil ->
            {:halt, {:error, {:missing_hash, key}}}

          ^expected_value ->
            {:cont, :ok}

          _ ->
            {:halt, {:error, {:hash_mismatch, key}}}
        end
      end)
    else
      {:error, :not_found} -> {:error, :signature_not_found}
    end
  end

  defp hydrate_signature(signature) do
    signature_id = stable_signature_id(signature.namespace, signature.name, signature.version)
    Map.put(signature, :signature_id, signature_id)
  end

  defp hashes_for(signature) do
    schema_blob = %{
      input_schema: Map.get(signature, :input_schema) || Map.get(signature, "input_schema"),
      output_schema: Map.get(signature, :output_schema) || Map.get(signature, "output_schema")
    }

    prompt_template =
      Map.get(signature, :prompt_template) || Map.get(signature, "prompt_template") || ""

    program_template =
      Map.get(signature, :program_template) || Map.get(signature, "program_template") || ""

    %{
      schema_hash: stable_hash(schema_blob),
      prompt_hash: stable_hash(prompt_template),
      program_hash: stable_hash(program_template)
    }
  end

  defp stable_hash(value) do
    value
    |> canonicalize()
    |> :erlang.term_to_binary()
    |> then(&:crypto.hash(:sha256, &1))
    |> Base.encode16(case: :lower)
  end

  defp canonicalize(%{} = map) do
    map
    |> Enum.map(fn {key, value} -> {to_string(key), canonicalize(value)} end)
    |> Enum.sort_by(&elem(&1, 0))
  end

  defp canonicalize(list) when is_list(list), do: Enum.map(list, &canonicalize/1)
  defp canonicalize(value), do: value
end
