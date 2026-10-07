use std::path::PathBuf;

use psionic_train::{
    build_compiled_agent_tailnet_node_bundle, compiled_agent_external_contributor_profile_from_id,
    compiled_agent_tailnet_archlinux_node_bundle_fixture_path,
    compiled_agent_tailnet_m5_node_bundle_fixture_path, write_compiled_agent_tailnet_node_bundle,
    CompiledAgentExternalContributorProfile,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut profile = None;
    let mut output = None;
    let mut print_only = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => {
                let value = args.next().ok_or("missing value for --profile")?;
                profile = compiled_agent_external_contributor_profile_from_id(&value);
                if profile.is_none() {
                    return Err(format!("unsupported profile `{value}`").into());
                }
            }
            "--output" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("missing value for --output")?,
                ));
            }
            "--print" => {
                print_only = true;
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }

    let profile = profile.ok_or("missing required --profile")?;
    if print_only {
        let bundle = build_compiled_agent_tailnet_node_bundle(profile)?;
        println!("{}", serde_json::to_string_pretty(&bundle)?);
        return Ok(());
    }

    let output = output.unwrap_or_else(|| default_output_path(profile));
    let bundle = write_compiled_agent_tailnet_node_bundle(&output, profile)?;
    println!(
        "wrote compiled-agent tailnet node bundle profile={} path={} digest={}",
        profile.profile_id(),
        output.display(),
        bundle.bundle_digest
    );
    Ok(())
}

fn default_output_path(profile: CompiledAgentExternalContributorProfile) -> PathBuf {
    match profile {
        CompiledAgentExternalContributorProfile::TailnetM5Mlx => {
            compiled_agent_tailnet_m5_node_bundle_fixture_path()
        }
        CompiledAgentExternalContributorProfile::TailnetArchlinuxRtx4080Cuda => {
            compiled_agent_tailnet_archlinux_node_bundle_fixture_path()
        }
        CompiledAgentExternalContributorProfile::ExternalAlpha => {
            compiled_agent_tailnet_archlinux_node_bundle_fixture_path()
        }
    }
}
