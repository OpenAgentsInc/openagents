pub mod github;
pub mod scramble;

pub use github::{github_callback, github_login};
pub use scramble::{scramble_callback, scramble_login, scramble_signup};
