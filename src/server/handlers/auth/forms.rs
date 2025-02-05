use serde::Deserialize;

// Custom deserializer for HTML checkbox
pub fn deserialize_checkbox<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // HTML checkboxes only send a value when checked
    // If the field is missing, it means unchecked
    Option::<String>::deserialize(deserializer).map(|x| x.is_some())
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SignupForm {
    pub email: String,
    pub password: String,
    #[serde(rename = "password-confirm")]
    pub password_confirmation: String,
    #[serde(rename = "terms", deserialize_with = "deserialize_checkbox", default)]
    pub terms_accepted: bool,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
    #[serde(
        rename = "remember-me",
        deserialize_with = "deserialize_checkbox",
        default
    )]
    pub remember_me: bool,
}

impl LoginForm {
    pub fn validate(&self) -> Result<(), String> {
        // Basic email validation
        if !self.email.contains('@') {
            return Err("Invalid email format".to_string());
        }

        // Basic password validation
        if self.password.len() < 8 {
            return Err("Password must be at least 8 characters".to_string());
        }

        Ok(())
    }
}
