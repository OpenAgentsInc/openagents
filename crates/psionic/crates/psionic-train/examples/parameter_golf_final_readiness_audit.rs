use std::{env, fs, path::PathBuf};

use psionic_train::{
    build_parameter_golf_final_readiness_audit_report, ParameterGolfFinalPrBundleReport,
    ParameterGolfFinalReadinessAuditReport, ParameterGolfLocalCloneDryRunReport,
    ParameterGolfRecordCandidateCampaignReport,
};

fn main() {
    let mut args = env::args().skip(1);
    let campaign_report_path = PathBuf::from(args.next().expect(
        "usage: parameter_golf_final_readiness_audit <campaign_report.json> <final_pr_bundle.json> <local_clone_dry_run.json> <output.json>",
    ));
    let final_pr_bundle_path =
        PathBuf::from(args.next().expect("missing final PR bundle json path"));
    let local_clone_dry_run_path =
        PathBuf::from(args.next().expect("missing local clone dry run json path"));
    let output_path = PathBuf::from(args.next().expect("missing output json path"));
    if args.next().is_some() {
        panic!("unexpected extra arguments");
    }

    let campaign_report: ParameterGolfRecordCandidateCampaignReport =
        serde_json::from_slice(&fs::read(&campaign_report_path).expect("read campaign report"))
            .expect("decode campaign report");
    let final_pr_bundle: ParameterGolfFinalPrBundleReport = serde_json::from_slice(
        &fs::read(&final_pr_bundle_path).expect("read final PR bundle report"),
    )
    .expect("decode final PR bundle report");
    let local_clone_dry_run: ParameterGolfLocalCloneDryRunReport = serde_json::from_slice(
        &fs::read(&local_clone_dry_run_path).expect("read local clone dry run report"),
    )
    .expect("decode local clone dry run report");

    let report: ParameterGolfFinalReadinessAuditReport =
        build_parameter_golf_final_readiness_audit_report(
            &campaign_report,
            &final_pr_bundle,
            &local_clone_dry_run,
        )
        .expect("build final readiness audit report");
    fs::write(
        &output_path,
        serde_json::to_vec_pretty(&report).expect("encode final readiness audit report"),
    )
    .expect("write final readiness audit report");
    println!("wrote {} ({})", output_path.display(), report.report_digest);
}
