pub mod auth;
pub mod oauth;
pub mod user;

pub use auth::{
    login::{handle_login, login_page},
    signup::{handle_signup, signup_page},
};
pub use oauth::github::{github_callback, github_login};
pub use oauth::scramble::{scramble_callback, scramble_login, scramble_signup};
