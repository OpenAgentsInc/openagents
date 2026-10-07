use psionic_eval::{
    tassadar_numeric_opcode_ladder_report_path, write_tassadar_numeric_opcode_ladder_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report =
        write_tassadar_numeric_opcode_ladder_report(tassadar_numeric_opcode_ladder_report_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
