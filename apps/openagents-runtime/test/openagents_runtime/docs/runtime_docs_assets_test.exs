defmodule OpenAgentsRuntime.Docs.RuntimeDocsAssetsTest do
  use ExUnit.Case, async: true

  @project_root Path.expand("../../..", __DIR__)
  @docs_dir Path.join(@project_root, "docs")

  test "DEPLOY_GCP runbook includes required deploy and rollback procedures" do
    deploy_doc = File.read!(Path.join(@docs_dir, "DEPLOY_GCP.md"))

    assert deploy_doc =~ "Run migration + smoke gate"
    assert deploy_doc =~ "runtime-migrate"
    assert deploy_doc =~ "runtime-smoke"
    assert deploy_doc =~ "rollout undo statefulset/runtime"
    assert deploy_doc =~ "RUNTIME_SIGNATURE_SECRET"
  end

  test "OPERATIONS runbook includes daily checks and incident workflows" do
    ops_doc = File.read!(Path.join(@docs_dir, "OPERATIONS.md"))

    assert ops_doc =~ "Daily checks"
    assert ops_doc =~ "Incident triage checklist"
    assert ops_doc =~ "Lease steals spike"
    assert ops_doc =~ "Tool failures spike"
    assert ops_doc =~ "Secrets rotation"
  end
end
