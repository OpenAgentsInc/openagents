defmodule OpenAgentsRuntimeWeb.Plugs.LegacyWriteFreezeTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: false

  setup do
    previous = System.get_env("LEGACY_RUNTIME_WRITE_FREEZE")

    on_exit(fn ->
      if is_binary(previous) do
        System.put_env("LEGACY_RUNTIME_WRITE_FREEZE", previous)
      else
        System.delete_env("LEGACY_RUNTIME_WRITE_FREEZE")
      end
    end)

    :ok
  end

  test "blocks legacy write routes when freeze is enabled", %{conn: conn} do
    System.put_env("LEGACY_RUNTIME_WRITE_FREEZE", "true")

    conn =
      conn
      |> put_internal_auth()
      |> post(~p"/internal/v1/comms/delivery-events", %{})

    assert %{"error" => %{"code" => "write_path_frozen"}} = json_response(conn, 410)
  end

  test "allows read routes while freeze is enabled", %{conn: conn} do
    System.put_env("LEGACY_RUNTIME_WRITE_FREEZE", "true")

    conn =
      conn
      |> put_internal_auth()
      |> get(~p"/internal/v1/health")

    assert %{"status" => "ok"} = json_response(conn, 200)
  end
end
