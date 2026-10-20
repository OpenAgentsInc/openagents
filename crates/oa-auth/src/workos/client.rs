//! The WorkOS calls we make, over the official SDK (`workos` 3.5.0, MIT).
//!
//! Every call answers our own types. An API error becomes a [`Refusal`]
//! that names what the person can do and never carries a token, a code,
//! a password, or WorkOS's own message text.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use workos::SecretString;
use workos::organization_membership::{
    CreateOrganizationMembershipParams, CreateOrganizationMembershipParamsBody,
    ListOrganizationMembershipsParams, Role as MembershipRole, UpdateOrganizationMembershipParams,
    UpdateOrganizationMembershipParamsBody,
};
use workos::organizations::{
    CreateOrganizationParams, ListOrganizationsParams, UpdateOrganizationParams,
};
use workos::user_management::{
    AuthenticateWithCodeParams, AuthenticateWithEmailVerificationParams,
    AuthenticateWithMagicAuthParams, AuthenticateWithPasswordParams,
    AuthenticateWithRefreshTokenParams, ConfirmPasswordResetParams, CreateMagicAuthParams,
    CreateUserParams, CreateUserParamsBody, GetAuthorizationUrlParams, ListUsersParams, Password,
    ResetPasswordParams, RevokeSessionParams, UpdateUserParams, UpdateUserParamsBody,
};

use super::Settings;
use super::token::{Claims, Keys};

/// A person as WorkOS holds them: only the fields we read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub id: String,
    pub email: String,
    pub email_verified: bool,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub external_id: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl User {
    /// A display label: the name WorkOS has, else the email's local part.
    #[must_use]
    pub fn label(&self) -> String {
        let name = [self.first_name.as_deref(), self.last_name.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if !name.is_empty() {
            return name;
        }
        if super::is_synthetic(&self.email) {
            return "OpenAgents user".into();
        }
        self.email
            .split('@')
            .next()
            .filter(|local| !local.is_empty())
            .unwrap_or("OpenAgents user")
            .to_string()
    }

    fn from_sdk(user: workos::User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            email_verified: user.email_verified,
            first_name: user.first_name,
            last_name: user.last_name,
            external_id: user.external_id,
            metadata: user.metadata.unwrap_or_default(),
        }
    }
}

/// What a finished WorkOS sign-in gives: the User and their session's
/// tokens. The tokens are secrets; `Debug` never prints them.
#[derive(Clone)]
pub struct Authenticated {
    pub user: User,
    pub access_token: SecretString,
    pub refresh_token: SecretString,
    /// How they signed in (`Password`, `MagicAuth`, `GitHubOAuth`, ...).
    pub method: Option<String>,
    pub organization_id: Option<String>,
}

impl std::fmt::Debug for Authenticated {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Authenticated")
            .field("user", &self.user.id)
            .field("method", &self.method)
            .finish_non_exhaustive()
    }
}

/// One way to finish a sign-in at `POST /user_management/authenticate`.
pub enum Grant {
    /// The code a provider sign-in returned to [`super::CALLBACK_PATH`].
    Code {
        code: String,
    },
    Password {
        email: String,
        password: String,
    },
    /// The emailed one-time code.
    MagicCode {
        email: String,
        code: String,
    },
    /// The emailed verification code, after a sign-in that needed one.
    EmailVerification {
        code: String,
        pending: String,
    },
    Refresh {
        refresh_token: String,
    },
}

/// Why WorkOS didn't do what we asked. Nothing here is secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The email and password don't match, or the code is wrong or used.
    InvalidCredentials,
    /// The person must verify their email first. WorkOS has emailed a
    /// code; `pending` finishes the sign-in with it.
    EmailVerificationRequired {
        pending: String,
        email_verification_id: Option<String>,
        email: Option<String>,
    },
    /// The password is too short, too weak, or was found in a breach.
    WeakPassword,
    /// Another person already uses this email.
    EmailTaken,
    /// The email address isn't a usable one.
    InvalidEmail,
    /// A code expired or was already used.
    Expired,
    /// The sign-in needs a step we don't offer here (MFA, choosing an
    /// organization, SSO the organization requires).
    Unsupported(String),
    /// Too many attempts; try again after this many seconds.
    RateLimited(u64),
    /// Nothing by that ID.
    NotFound,
    /// WorkOS can't be reached, or answered with something unexpected.
    Unavailable,
}

impl Refusal {
    /// A stable code for our error envelope.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "invalid_credentials",
            Self::EmailVerificationRequired { .. } => "email_verification_required",
            Self::WeakPassword => "weak_password",
            Self::EmailTaken => "email_taken",
            Self::InvalidEmail => "invalid_email",
            Self::Expired => "code_expired",
            Self::Unsupported(_) => "sign_in_unsupported",
            Self::RateLimited(_) => "rate_limited",
            Self::NotFound => "not_found",
            Self::Unavailable => "sign_in_unavailable",
        }
    }

    /// What to tell the person, in plain words.
    #[must_use]
    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "That email and password don't match. Try again.",
            Self::EmailVerificationRequired { .. } => {
                "Check your email for a code to confirm your address."
            }
            Self::WeakPassword => {
                "Choose a stronger password: at least 10 characters, and not one that's been leaked."
            }
            Self::EmailTaken => "An account already uses that email. Log in instead.",
            Self::InvalidEmail => "Enter a valid email address.",
            Self::Expired => "That code expired or was already used. Ask for a new one.",
            Self::Unsupported(_) => "This account needs a sign-in step we don't offer yet.",
            Self::RateLimited(_) => "Too many tries. Wait a minute and try again.",
            Self::NotFound => "We couldn't find that.",
            Self::Unavailable => "Sign-in isn't available right now. Try again in a minute.",
        }
    }

    /// The HTTP status our routes answer with.
    #[must_use]
    pub fn status(&self) -> u16 {
        match self {
            Self::InvalidCredentials => 401,
            Self::EmailVerificationRequired { .. } => 403,
            Self::WeakPassword | Self::InvalidEmail => 422,
            Self::EmailTaken => 409,
            Self::Expired => 400,
            Self::Unsupported(_) => 403,
            Self::RateLimited(_) => 429,
            Self::NotFound => 404,
            Self::Unavailable => 503,
        }
    }

    pub(crate) fn from_sdk(error: &workos::Error) -> Self {
        let Some(api) = error.api() else {
            return Self::Unavailable;
        };
        let body: serde_json::Value = serde_json::from_slice(&api.raw_body).unwrap_or_default();
        let code = api.code.as_deref().unwrap_or_default();
        let inner_codes: Vec<String> = body["errors"]
            .as_array()
            .map(|errors| {
                errors
                    .iter()
                    .filter_map(|e| e["code"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let any =
            |needle: &str| code.contains(needle) || inner_codes.iter().any(|c| c.contains(needle));
        if api.status == 429 {
            return Self::RateLimited(api.retry_after.map_or(60, |after| after.as_secs().max(1)));
        }
        if code == "email_verification_required" {
            return match body["pending_authentication_token"].as_str() {
                Some(pending) => Self::EmailVerificationRequired {
                    pending: pending.to_string(),
                    email_verification_id: body["email_verification_id"]
                        .as_str()
                        .map(str::to_string),
                    email: body["email"].as_str().map(str::to_string),
                },
                None => Self::Unavailable,
            };
        }
        if any("password_strength")
            || any("password_pwned")
            || any("password_too_short")
            || any("weak_password")
            || any("password") && api.status == 422
        {
            return Self::WeakPassword;
        }
        if any("email_not_available") || any("user_already_exists") || any("email_taken") {
            return Self::EmailTaken;
        }
        if any("email_invalid") || any("invalid_email") {
            return Self::InvalidEmail;
        }
        if code.contains("expired")
            || code == "invalid_one_time_code"
                && body["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("expired"))
        {
            return Self::Expired;
        }
        match code {
            "invalid_credentials"
            | "invalid_grant"
            | "invalid_one_time_code"
            | "one_time_code_previously_used" => {
                return Self::InvalidCredentials;
            }
            "organization_selection_required"
            | "mfa_enrollment"
            | "mfa_challenge"
            | "sso_required"
            | "organization_authentication_methods_required" => {
                return Self::Unsupported(code.to_string());
            }
            _ => {}
        }
        match api.status {
            404 => Self::NotFound,
            400 | 401 | 403 => Self::InvalidCredentials,
            _ => Self::Unavailable,
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Refusal {}

/// An Organization, as far as we read one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub external_id: Option<String>,
}

/// A membership, as far as we read one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Membership {
    pub id: String,
    pub user_id: String,
    pub organization_id: String,
    pub role: String,
}

/// A User's linked provider sign-in, such as `("GithubOAuth", "14167547")`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub provider: String,
    pub idp_id: String,
}

/// What [`Workos::create_user`] sets on a new User.
#[derive(Clone, Debug, Default)]
pub struct NewUser {
    pub email: String,
    pub password: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email_verified: Option<bool>,
    pub external_id: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// What [`Workos::update_user`] changes on a User; `None` leaves a field.
#[derive(Clone, Debug, Default)]
pub struct UserChange {
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub external_id: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

/// The WorkOS client for one environment.
#[derive(Clone)]
pub struct Workos {
    client: Arc<workos::Client>,
    keys: Keys,
    settings: Arc<Settings>,
    /// The environment's organization role slugs, read once.
    roles: Arc<tokio::sync::OnceCell<Vec<String>>>,
}

impl std::fmt::Debug for Workos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workos")
            .field("client_id", &self.settings.client_id)
            .field("mode", &self.settings.mode())
            .finish_non_exhaustive()
    }
}

type Result<T> = std::result::Result<T, Refusal>;

impl Workos {
    /// A client for `settings`, reading its API key.
    pub fn new(settings: Settings) -> std::result::Result<Self, String> {
        let key = settings.api_key()?;
        Ok(Self::with_key(settings, &key))
    }

    /// A client for `settings` with `api_key` given directly.
    #[must_use]
    pub fn with_key(settings: Settings, api_key: &str) -> Self {
        let mut builder = workos::Client::builder()
            .api_key(api_key)
            .client_id(settings.client_id.clone())
            .timeout(Duration::from_secs(10))
            .max_retries(1)
            .user_agent("openagents-oa-auth");
        if let Some(base) = &settings.base_url {
            builder = builder.base_url(base.trim_end_matches('/'));
        }
        let client = Arc::new(builder.build());
        let keys = Keys::new(client.clone());
        Self {
            client,
            keys,
            settings: Arc::new(settings),
            roles: Arc::new(tokio::sync::OnceCell::new()),
        }
    }

    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The SDK client, for the sealed-session helpers.
    #[must_use]
    pub fn sdk(&self) -> &workos::Client {
        &self.client
    }

    /// The organization role slug for `role`: the configured one when the
    /// environment has it, else the nearest it has (an owner is an admin
    /// where there's no `owner` role), so memberships don't drift.
    pub async fn role_slug(&self, role: tenancy::Role) -> String {
        let wanted = self.settings.roles.slug(role).to_string();
        let have = self
            .roles
            .get_or_try_init(|| async {
                let list = self
                    .client
                    .authorization()
                    .list_environment_roles()
                    .await
                    .map_err(|error| Refusal::from_sdk(&error))?;
                let value = serde_json::to_value(list).map_err(|_| Refusal::Unavailable)?;
                Ok::<_, Refusal>(
                    value["data"]
                        .as_array()
                        .map(|roles| {
                            roles
                                .iter()
                                .filter_map(|role| role["slug"].as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default(),
                )
            })
            .await;
        let Ok(have) = have else {
            return wanted;
        };
        if have.is_empty() || have.contains(&wanted) {
            return wanted;
        }
        let fallback = match role {
            tenancy::Role::Owner => [&self.settings.roles.admin, &self.settings.roles.member],
            tenancy::Role::Admin => [&self.settings.roles.member, &self.settings.roles.member],
            tenancy::Role::Member => return wanted,
        };
        fallback
            .into_iter()
            .find(|slug| have.contains(slug))
            .cloned()
            .unwrap_or(wanted)
    }

    /// The JWKS cache behind [`Self::verify`].
    #[must_use]
    pub fn keys(&self) -> &Keys {
        &self.keys
    }

    /// Check a WorkOS access token's signature and expiry against the
    /// environment's keys, fetching them when the cache doesn't hold the
    /// token's key.
    pub async fn verify(&self, access_token: &str) -> Result<Claims> {
        self.keys.verify(access_token, false).await
    }

    /// The URL that starts a provider sign-in: an OAuth provider such as
    /// `GitHubOAuth`, or an organization's SSO.
    pub fn authorization_url(
        &self,
        provider: Option<&str>,
        organization: Option<&str>,
        redirect_uri: &str,
        state: &str,
        login_hint: Option<&str>,
    ) -> Result<String> {
        let mut params = GetAuthorizationUrlParams::new(redirect_uri);
        params.provider = provider.map(|name| {
            serde_json::from_value(serde_json::Value::String(name.to_string())).unwrap_or(
                workos::UserManagementAuthenticationProvider::Unknown(name.to_string()),
            )
        });
        params.organization_id = organization.map(str::to_string);
        params.state = Some(state.to_string());
        params.login_hint = login_hint.map(str::to_string);
        self.client
            .user_management()
            .get_authorization_url(params)
            .map_err(|_| Refusal::Unavailable)
    }

    /// Finish a sign-in.
    pub async fn authenticate(&self, grant: Grant) -> Result<Authenticated> {
        let api = self.client.user_management();
        let answer = match grant {
            Grant::Code { code } => {
                api.authenticate_with_code(AuthenticateWithCodeParams::new(code))
                    .await
            }
            Grant::Password { email, password } => {
                api.authenticate_with_password(AuthenticateWithPasswordParams::new(email, password))
                    .await
            }
            Grant::MagicCode { email, code } => {
                api.authenticate_with_magic_auth(AuthenticateWithMagicAuthParams::new(code, email))
                    .await
            }
            Grant::EmailVerification { code, pending } => {
                api.authenticate_with_email_verification(
                    AuthenticateWithEmailVerificationParams::new(code, pending),
                )
                .await
            }
            Grant::Refresh { refresh_token } => {
                api.authenticate_with_refresh_token(AuthenticateWithRefreshTokenParams::new(
                    refresh_token,
                ))
                .await
            }
        }
        .map_err(|error| Refusal::from_sdk(&error))?;
        Ok(Authenticated {
            user: User::from_sdk(answer.user),
            access_token: answer.access_token,
            refresh_token: answer.refresh_token,
            method: answer
                .authentication_method
                .and_then(|method| serde_json::to_value(method).ok())
                .and_then(|value| value.as_str().map(str::to_string)),
            organization_id: answer.organization_id,
        })
    }

    /// Make a User.
    pub async fn create_user(&self, new: NewUser) -> Result<User> {
        let mut body = CreateUserParamsBody::new(new.email);
        body.first_name = new.first_name;
        body.last_name = new.last_name;
        body.email_verified = new.email_verified;
        body.external_id = new.external_id;
        body.metadata = (!new.metadata.is_empty()).then_some(new.metadata);
        body.password = new
            .password
            .map(|password| Password::Plaintext { password });
        let made = self
            .client
            .user_management()
            .create_user(CreateUserParams::new(body))
            .await
            .map_err(|error| Refusal::from_sdk(&error))?;
        let value = serde_json::to_value(made).map_err(|_| Refusal::Unavailable)?;
        let user: workos::User = serde_json::from_value(value).map_err(|_| Refusal::Unavailable)?;
        Ok(User::from_sdk(user))
    }

    /// The User with `id`, or `None`.
    pub async fn user(&self, id: &str) -> Result<Option<User>> {
        found(self.client.user_management().get_user(id).await).map(|user| user.map(User::from_sdk))
    }

    /// The User whose external ID is `external_id`, or `None`.
    pub async fn user_by_external_id(&self, external_id: &str) -> Result<Option<User>> {
        found(
            self.client
                .user_management()
                .get_user_by_external_id(external_id)
                .await,
        )
        .map(|user| user.map(User::from_sdk))
    }

    /// The Users with exactly this email (WorkOS keeps emails unique, so
    /// at most one).
    pub async fn users_by_email(&self, email: &str) -> Result<Vec<User>> {
        let mut params = ListUsersParams::default();
        params.email = Some(email.trim().to_ascii_lowercase());
        params.limit = Some(10);
        let list = self
            .client
            .user_management()
            .list_users(params)
            .await
            .map_err(|error| Refusal::from_sdk(&error))?;
        Ok(list.data.into_iter().map(User::from_sdk).collect())
    }

    /// Change a User.
    pub async fn update_user(&self, id: &str, change: UserChange) -> Result<User> {
        let mut body = UpdateUserParamsBody::default();
        body.email = change.email;
        body.email_verified = change.email_verified;
        body.external_id = change.external_id;
        body.metadata = change.metadata;
        self.client
            .user_management()
            .update_user(id, UpdateUserParams::new(body))
            .await
            .map(User::from_sdk)
            .map_err(|error| Refusal::from_sdk(&error))
    }

    /// Delete a User. One already gone is fine.
    pub async fn delete_user(&self, id: &str) -> Result<()> {
        match self.client.user_management().delete_user(id).await {
            Ok(()) => Ok(()),
            Err(error) if error.is_not_found() => Ok(()),
            Err(error) => Err(Refusal::from_sdk(&error)),
        }
    }

    /// The provider sign-ins linked to a User.
    pub async fn identities(&self, id: &str) -> Result<Vec<Identity>> {
        let items = self
            .client
            .user_management()
            .get_user_identities(id)
            .await
            .map_err(|error| Refusal::from_sdk(&error))?;
        Ok(items
            .into_iter()
            .map(|item| Identity {
                provider: item.provider.as_str().to_string(),
                idp_id: item.idp_id,
            })
            .collect())
    }

    /// Email a one-time sign-in code. WorkOS answers the code too; only a
    /// test check against a staging environment ever reads it.
    pub async fn send_code(&self, email: &str) -> Result<SecretString> {
        let made = self
            .client
            .user_management()
            .create_magic_auth(CreateMagicAuthParams::new(
                workos::CreateMagicCodeAndReturn {
                    email: email.trim().to_string(),
                    invitation_token: None,
                    ip_address: None,
                    user_agent: None,
                    radar_auth_attempt_id: None,
                    signals_id: None,
                },
            ))
            .await
            .map_err(|error| Refusal::from_sdk(&error))?;
        Ok(made.code)
    }

    /// The code of an email verification WorkOS sent, for test checks.
    pub async fn email_verification_code(&self, id: &str) -> Result<SecretString> {
        self.client
            .user_management()
            .get_email_verification(id)
            .await
            .map(|verification| verification.code)
            .map_err(|error| Refusal::from_sdk(&error))
    }

    /// Email a password-reset link. An unknown email is not an error to
    /// the caller: the page says the same thing either way.
    pub async fn send_password_reset(&self, email: &str) -> Result<()> {
        match self
            .client
            .user_management()
            .reset_password(ResetPasswordParams::new(workos::CreatePasswordResetToken {
                email: email.trim().to_string(),
            }))
            .await
        {
            Ok(_) => Ok(()),
            Err(error) if error.is_not_found() => Ok(()),
            Err(error) => match Refusal::from_sdk(&error) {
                Refusal::NotFound | Refusal::InvalidCredentials => Ok(()),
                other => Err(other),
            },
        }
    }

    /// Set a new password with the token from the reset email.
    pub async fn reset_password(&self, token: &str, password: &str) -> Result<User> {
        let answer = self
            .client
            .user_management()
            .confirm_password_reset(ConfirmPasswordResetParams::new(
                workos::CreatePasswordReset {
                    token: token.into(),
                    new_password: password.into(),
                },
            ))
            .await
            .map_err(|error| match Refusal::from_sdk(&error) {
                Refusal::InvalidCredentials | Refusal::NotFound => Refusal::Expired,
                other => other,
            })?;
        let value = serde_json::to_value(answer).map_err(|_| Refusal::Unavailable)?;
        let user = value.get("user").cloned().unwrap_or(value);
        let user: workos::User = serde_json::from_value(user).map_err(|_| Refusal::Unavailable)?;
        Ok(User::from_sdk(user))
    }

    /// End a WorkOS session everywhere it's used.
    pub async fn revoke_session(&self, session_id: &str) -> Result<()> {
        match self
            .client
            .user_management()
            .revoke_session(RevokeSessionParams::new(workos::RevokeSession {
                session_id: session_id.to_string(),
            }))
            .await
        {
            Ok(()) => Ok(()),
            Err(error) if error.is_not_found() => Ok(()),
            Err(error) => Err(Refusal::from_sdk(&error)),
        }
    }

    /// The organization whose verified domain is `domain`, for SSO.
    pub async fn organization_for_domain(&self, domain: &str) -> Result<Option<Organization>> {
        let mut params = ListOrganizationsParams::default();
        params.domains = Some(vec![domain.trim().to_ascii_lowercase()]);
        params.limit = Some(2);
        let list = self
            .client
            .organizations()
            .list_organizations(params)
            .await
            .map_err(|error| Refusal::from_sdk(&error))?;
        Ok(list.data.into_iter().next().map(organization))
    }

    /// The Organization whose external ID is `external_id`, or `None`.
    pub async fn organization_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<Option<Organization>> {
        found(
            self.client
                .organizations()
                .get_organization_by_external_id(external_id)
                .await,
        )
        .map(|org| org.map(organization))
    }

    /// Make an Organization.
    #[allow(deprecated)] // `domains` is listed, never set.
    pub async fn create_organization(&self, name: &str, external_id: &str) -> Result<Organization> {
        self.client
            .organizations()
            .create_organization(CreateOrganizationParams::new(workos::OrganizationInput {
                name: name.to_string(),
                allow_profiles_outside_organization: None,
                domains: None,
                domain_data: None,
                metadata: None,
                external_id: Some(external_id.to_string()),
            }))
            .await
            .map(organization)
            .map_err(|error| Refusal::from_sdk(&error))
    }

    /// Rename an Organization.
    #[allow(deprecated)] // `domains` is listed, never set.
    pub async fn rename_organization(&self, id: &str, name: &str) -> Result<Organization> {
        self.client
            .organizations()
            .update_organization(
                id,
                UpdateOrganizationParams::new(workos::UpdateOrganization {
                    name: Some(name.to_string()),
                    allow_profiles_outside_organization: None,
                    domains: None,
                    domain_data: None,
                    stripe_customer_id: None,
                    metadata: None,
                    external_id: None,
                }),
            )
            .await
            .map(organization)
            .map_err(|error| Refusal::from_sdk(&error))
    }

    /// Delete an Organization. One already gone is fine.
    pub async fn delete_organization(&self, id: &str) -> Result<()> {
        match self.client.organizations().delete_organization(id).await {
            Ok(()) => Ok(()),
            Err(error) if error.is_not_found() => Ok(()),
            Err(error) => Err(Refusal::from_sdk(&error)),
        }
    }

    /// Every membership of an Organization (up to 100, our seat cap).
    pub async fn memberships(&self, organization_id: &str) -> Result<Vec<Membership>> {
        let mut params = ListOrganizationMembershipsParams::default();
        params.organization_id = Some(organization_id.to_string());
        params.limit = Some(100);
        let list = self
            .client
            .organization_membership()
            .list_organization_memberships(params)
            .await
            .map_err(|error| Refusal::from_sdk(&error))?;
        let value = serde_json::to_value(list).map_err(|_| Refusal::Unavailable)?;
        Ok(value["data"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        Some(Membership {
                            id: row["id"].as_str()?.to_string(),
                            user_id: row["user_id"].as_str()?.to_string(),
                            organization_id: row["organization_id"].as_str()?.to_string(),
                            role: row["role"]["slug"].as_str().unwrap_or_default().to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Add a User to an Organization with `role`, falling back to the
    /// environment's default role when that slug doesn't exist there.
    pub async fn add_membership(
        &self,
        user_id: &str,
        organization_id: &str,
        role: &str,
    ) -> Result<Membership> {
        let mut body = CreateOrganizationMembershipParamsBody::new(user_id, organization_id);
        body.role = Some(MembershipRole::Single {
            role_slug: role.to_string(),
        });
        let api = self.client.organization_membership();
        let made = match api
            .create_organization_membership(CreateOrganizationMembershipParams::new(body))
            .await
        {
            Ok(made) => made,
            Err(error) if is_role_problem(&error) => api
                .create_organization_membership(CreateOrganizationMembershipParams::new(
                    CreateOrganizationMembershipParamsBody::new(user_id, organization_id),
                ))
                .await
                .map_err(|error| Refusal::from_sdk(&error))?,
            Err(error) => return Err(Refusal::from_sdk(&error)),
        };
        Ok(Membership {
            id: made.id,
            user_id: made.user_id,
            organization_id: made.organization_id,
            role: made.role.slug,
        })
    }

    /// Change a membership's role; a role the environment lacks is left.
    pub async fn set_membership_role(&self, membership_id: &str, role: &str) -> Result<()> {
        let mut body = UpdateOrganizationMembershipParamsBody::new();
        body.role = Some(MembershipRole::Single {
            role_slug: role.to_string(),
        });
        match self
            .client
            .organization_membership()
            .update_organization_membership(
                membership_id,
                UpdateOrganizationMembershipParams::new(body),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(error) if is_role_problem(&error) => Ok(()),
            Err(error) => Err(Refusal::from_sdk(&error)),
        }
    }

    /// Remove a membership. One already gone is fine.
    pub async fn remove_membership(&self, membership_id: &str) -> Result<()> {
        match self
            .client
            .organization_membership()
            .delete_organization_membership(membership_id)
            .await
        {
            Ok(()) => Ok(()),
            Err(error) if error.is_not_found() => Ok(()),
            Err(error) => Err(Refusal::from_sdk(&error)),
        }
    }
}

fn is_role_problem(error: &workos::Error) -> bool {
    error.status() == Some(422) || error.code().is_some_and(|code| code.contains("role"))
}

fn organization(org: workos::Organization) -> Organization {
    Organization {
        id: org.id,
        name: org.name,
        external_id: org.external_id,
    }
}

fn found<T>(answer: std::result::Result<T, workos::Error>) -> Result<Option<T>> {
    match answer {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.is_not_found() => Ok(None),
        Err(error) => Err(Refusal::from_sdk(&error)),
    }
}
