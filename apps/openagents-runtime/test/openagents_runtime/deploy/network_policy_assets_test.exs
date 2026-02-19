defmodule OpenAgentsRuntime.Deploy.NetworkPolicyAssetsTest do
  use ExUnit.Case, async: true

  @project_root Path.expand("../../..", __DIR__)
  @base_dir Path.join(@project_root, "deploy/k8s/base")
  @jobs_dir Path.join(@project_root, "deploy/jobs")

  test "kustomization includes runtime ingress network policy" do
    {:ok, kustomization} = YamlElixir.read_from_file(Path.join(@base_dir, "kustomization.yaml"))
    resources = Map.get(kustomization, "resources", [])

    assert "networkpolicy-ingress.yaml" in resources
  end

  test "network policy restricts BEAM ports and HTTP ingress sources" do
    {:ok, network_policy} =
      YamlElixir.read_from_file(Path.join(@base_dir, "networkpolicy-ingress.yaml"))

    assert network_policy["kind"] == "NetworkPolicy"

    ingress_rules = get_in(network_policy, ["spec", "ingress"])
    assert is_list(ingress_rules)
    assert length(ingress_rules) == 2

    beam_rule = Enum.at(ingress_rules, 0)
    http_rule = Enum.at(ingress_rules, 1)

    beam_ports =
      beam_rule
      |> Map.get("ports", [])
      |> Enum.map(&Map.get(&1, "port"))
      |> Enum.sort()

    assert beam_ports == [4369, 9000]

    http_ports =
      http_rule
      |> Map.get("ports", [])
      |> Enum.map(&Map.get(&1, "port"))

    assert http_ports == [4000]

    assert Enum.any?(http_rule["from"], fn source ->
             get_in(source, ["podSelector", "matchLabels", "app.kubernetes.io/name"]) ==
               "openagents-com"
           end)

    assert Enum.any?(http_rule["from"], fn source ->
             get_in(source, ["namespaceSelector", "matchLabels", "openagents.io/control-plane"]) ==
               "true"
           end)
  end

  test "smoke job pod label is authorized runtime HTTP client" do
    {:ok, smoke_job} = YamlElixir.read_from_file(Path.join(@jobs_dir, "smoke-job.yaml"))

    assert get_in(smoke_job, ["metadata", "labels", "openagents.io/runtime-http-client"]) ==
             "true"

    assert get_in(smoke_job, [
             "spec",
             "template",
             "metadata",
             "labels",
             "openagents.io/runtime-http-client"
           ]) == "true"
  end
end
