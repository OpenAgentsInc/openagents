use psionic_router::{
    tassadar_internal_external_delegation_route_matrix_path,
    write_tassadar_internal_external_delegation_route_matrix,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_internal_external_delegation_route_matrix_path();
    let matrix = write_tassadar_internal_external_delegation_route_matrix(&path)?;
    println!("wrote {} to {}", matrix.matrix_id, path.display());
    Ok(())
}
