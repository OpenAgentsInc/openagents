pub mod auth;
pub mod oauth;
pub mod user;

pub use auth::{
    callback,
    login::{handle_login, login_page},
    session::clear_session_and_redirect,
    signup::{handle_signup, signup_page},
};
pub use oauth::github::{github_callback, github_login};
pub use oauth::scramble::{scramble_callback, scramble_login, scramble_signup};