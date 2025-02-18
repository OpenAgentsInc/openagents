pub mod oauth;
pub mod user;

pub use oauth::github::{github_callback, github_login};
pub use oauth::scramble::{scramble_callback, scramble_login, scramble_signup};
