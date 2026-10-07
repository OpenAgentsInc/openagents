use psionic_router::{
    tassadar_negative_invocation_route_audit_path, write_tassadar_negative_invocation_route_audit,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_negative_invocation_route_audit_path();
    let audit = write_tassadar_negative_invocation_route_audit(&path)?;
    println!("wrote {} to {}", audit.audit_id, path.display());
    Ok(())
}
