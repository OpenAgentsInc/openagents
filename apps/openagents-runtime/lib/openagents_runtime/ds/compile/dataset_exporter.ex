defmodule OpenAgentsRuntime.DS.Compile.DatasetExporter do
  @moduledoc """
  Dataset export helper boundary.
  """

  @spec export([map()]) :: {:ok, %{count: non_neg_integer()}}
  def export(examples) when is_list(examples) do
    {:ok, %{count: length(examples)}}
  end
end
