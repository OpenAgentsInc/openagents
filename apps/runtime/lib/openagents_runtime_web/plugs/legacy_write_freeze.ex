defmodule OpenAgentsRuntimeWeb.Plugs.LegacyWriteFreeze do
  @moduledoc """
  Freezes legacy runtime authority mutation routes during Rust cutover.
  """

  import Plug.Conn

  @write_methods MapSet.new(~w(POST PUT PATCH DELETE))

  @spec init(term()) :: term()
  def init(opts), do: opts

  @spec call(Plug.Conn.t(), term()) :: Plug.Conn.t()
  def call(conn, _opts) do
    if freeze_enabled?() and MapSet.member?(@write_methods, conn.method) do
      body = %{
        error: %{
          code: "write_path_frozen",
          message:
            "Legacy runtime authority writes are frozen. Route writes through Rust runtime authority."
        }
      }

      conn
      |> put_resp_content_type("application/json")
      |> send_resp(410, Jason.encode!(body))
      |> halt()
    else
      conn
    end
  end

  defp freeze_enabled? do
    case System.get_env("LEGACY_RUNTIME_WRITE_FREEZE", "false") do
      value when is_binary(value) ->
        String.downcase(String.trim(value)) in ["1", "true", "yes", "on"]

      _other ->
        false
    end
  end
end
