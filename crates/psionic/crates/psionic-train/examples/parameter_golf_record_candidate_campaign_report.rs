use std::{env, fs, path::PathBuf};

use psionic_eval::ParameterGolfSubmissionPromotionReceipt;
use psionic_train::{
    build_parameter_golf_record_candidate_campaign_report,
    ParameterGolfRecordCandidateCampaignEvidence, ParameterGolfRecordCandidateFrozenConfig,
    ParameterGolfSubmissionRunEvidenceReport,
};

fn main() {
    let mut args = env::args().skip(1);
    let campaign_id = args.next().expect(
        "usage: parameter_golf_record_candidate_campaign_report <campaign_id> <frozen_config.json> <output.json> <run_evidence_1.json> <promotion_receipt_1.json> [<run_evidence_2.json> <promotion_receipt_2.json> ...]",
    );
    let frozen_config_path = PathBuf::from(args.next().expect("missing frozen config json path"));
    let output_path = PathBuf::from(args.next().expect("missing output json path"));
    let remaining = args.collect::<Vec<_>>();
    if remaining.is_empty() || remaining.len() % 2 != 0 {
        panic!(
            "expected one or more <run_evidence.json> <promotion_receipt.json> pairs after the output path"
        );
    }

    let frozen_config: ParameterGolfRecordCandidateFrozenConfig = serde_json::from_slice(
        &fs::read(&frozen_config_path).expect("read frozen candidate config"),
    )
    .expect("decode frozen candidate config");

    let mut evidence = Vec::new();
    for pair in remaining.chunks(2) {
        let run_evidence_path = PathBuf::from(&pair[0]);
        let promotion_receipt_path = PathBuf::from(&pair[1]);
        let submission_run_evidence: ParameterGolfSubmissionRunEvidenceReport =
            serde_json::from_slice(&fs::read(&run_evidence_path).expect("read run evidence"))
                .expect("decode run evidence");
        let promotion_receipt: ParameterGolfSubmissionPromotionReceipt = serde_json::from_slice(
            &fs::read(&promotion_receipt_path).expect("read promotion receipt"),
        )
        .expect("decode promotion receipt");
        evidence.push(ParameterGolfRecordCandidateCampaignEvidence {
            evidence_id: format!("evidence-{}", evidence.len() + 1),
            submission_run_evidence,
            promotion_receipt,
        });
    }

    let report = build_parameter_golf_record_candidate_campaign_report(
        campaign_id,
        frozen_config,
        evidence.as_slice(),
    )
    .expect("build record candidate campaign report");
    fs::write(
        &output_path,
        serde_json::to_vec_pretty(&report).expect("encode record candidate campaign report"),
    )
    .expect("write record candidate campaign report");
    println!("wrote {} ({})", output_path.display(), report.report_digest);
}
