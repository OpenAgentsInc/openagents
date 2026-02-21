defmodule OpenAgentsRuntime.Deploy.JobsAssetsTest do
  use ExUnit.Case, async: true

  @project_root Path.expand("../../..", __DIR__)
  @jobs_dir Path.join(@project_root, "deploy/jobs")

  test "migration and smoke jobs are defined with expected release commands and secrets" do
    {:ok, migration_job} = YamlElixir.read_from_file(Path.join(@jobs_dir, "migration-job.yaml"))
    {:ok, smoke_job} = YamlElixir.read_from_file(Path.join(@jobs_dir, "smoke-job.yaml"))

    assert migration_job["kind"] == "Job"
    assert smoke_job["kind"] == "Job"

    migration_container = first_container(migration_job)
    smoke_container = first_container(smoke_job)

    assert migration_container["command"] == [
             "bin/openagents_runtime",
             "eval",
             "OpenAgentsRuntime.Release.migrate_and_verify!()"
           ]

    assert smoke_container["command"] == [
             "bin/openagents_runtime",
             "eval",
             "OpenAgentsRuntime.Deploy.Smoke.run!(base_url: System.get_env(\"SMOKE_BASE_URL\"), tail_ms: 150)"
           ]

    assert required_secret_keys(migration_container) == [
             "DATABASE_URL",
             "RUNTIME_SIGNATURE_SECRET",
             "SECRET_KEY_BASE"
           ]

    assert required_secret_keys(smoke_container) == [
             "DATABASE_URL",
             "RUNTIME_SIGNATURE_SECRET",
             "SECRET_KEY_BASE"
           ]
  end

  test "post-deploy gate runner script is executable and references both jobs" do
    script_path = Path.join(@jobs_dir, "run-postdeploy-gate.sh")
    script = File.read!(script_path)
    assert String.starts_with?(script, "#!/usr/bin/env bash")
    assert script =~ "runtime-migrate"
    assert script =~ "runtime-smoke"
  end

  defp first_container(job_doc) do
    job_doc
    |> get_in(["spec", "template", "spec", "containers"])
    |> List.first()
  end

  defp required_secret_keys(container) do
    container
    |> Map.get("env", [])
    |> Enum.flat_map(fn env_var ->
      case get_in(env_var, ["valueFrom", "secretKeyRef", "key"]) do
        nil -> []
        key -> [key]
      end
    end)
    |> Enum.sort()
  end
end
