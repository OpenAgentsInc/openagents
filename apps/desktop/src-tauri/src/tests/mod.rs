// Test modules organization

pub mod unit;
pub mod integration;
pub mod helpers;

// Authentication integration tests (Phase 1: Foundation & Analysis)
pub mod auth_integration_baseline;
pub mod convex_auth_flow;

// Phase 3: JWT Integration tests
pub mod jwt_integration_phase3;