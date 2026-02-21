use openagents_app_state::{
    AppAction, AppState, AuthStatus, AuthUser, SessionLifecycleStatus, SessionSnapshot,
    apply_action,
};

fn sample_user() -> AuthUser {
    AuthUser {
        user_id: "user_123".to_string(),
        email: "dev@example.com".to_string(),
        name: "Dev Example".to_string(),
        workos_id: "workos_user_123".to_string(),
    }
}

fn sample_session() -> SessionSnapshot {
    SessionSnapshot {
        session_id: "sess_123".to_string(),
        user_id: "user_123".to_string(),
        device_id: "device:web".to_string(),
        token_name: "web:openagents-web-shell".to_string(),
        active_org_id: "org_123".to_string(),
        status: SessionLifecycleStatus::Active,
        reauth_required: false,
        issued_at: Some("2026-02-21T00:00:00Z".to_string()),
        access_expires_at: Some("2026-02-21T01:00:00Z".to_string()),
        refresh_expires_at: Some("2026-03-21T00:00:00Z".to_string()),
    }
}

#[test]
fn auth_challenge_and_verify_flow_transitions_to_signed_in() {
    let mut state = AppState::default();
    let _ = apply_action(
        &mut state,
        AppAction::AuthChallengeRequested {
            email: "dev@example.com".to_string(),
        },
    );
    assert_eq!(state.auth.status, AuthStatus::SendingCode);
    assert_eq!(state.auth.email.as_deref(), Some("dev@example.com"));

    let _ = apply_action(
        &mut state,
        AppAction::AuthChallengeAccepted {
            email: "dev@example.com".to_string(),
            challenge_id: "challenge_123".to_string(),
        },
    );
    assert_eq!(state.auth.status, AuthStatus::AwaitingCode);
    assert_eq!(state.auth.challenge_id.as_deref(), Some("challenge_123"));

    let _ = apply_action(&mut state, AppAction::AuthVerifyRequested);
    assert_eq!(state.auth.status, AuthStatus::VerifyingCode);

    let _ = apply_action(
        &mut state,
        AppAction::AuthSessionEstablished {
            user: sample_user(),
            session: sample_session(),
            token_type: "Bearer".to_string(),
            access_token: "oa_at_abc".to_string(),
            refresh_token: "oa_rt_abc".to_string(),
        },
    );
    assert_eq!(state.auth.status, AuthStatus::SignedIn);
    assert!(state.auth.has_tokens());
    assert!(state.auth.has_active_session());
    assert_eq!(state.auth.last_error, None);
}

#[test]
fn auth_refresh_failure_requires_reauth_and_clears_tokens() {
    let mut state = AppState::default();
    let _ = apply_action(
        &mut state,
        AppAction::AuthSessionEstablished {
            user: sample_user(),
            session: sample_session(),
            token_type: "Bearer".to_string(),
            access_token: "oa_at_abc".to_string(),
            refresh_token: "oa_rt_abc".to_string(),
        },
    );
    assert!(state.auth.has_tokens());

    let _ = apply_action(
        &mut state,
        AppAction::AuthReauthRequired {
            message: "Reauthentication required.".to_string(),
        },
    );
    assert_eq!(state.auth.status, AuthStatus::ReauthRequired);
    assert!(!state.auth.has_tokens());
    assert!(!state.auth.has_active_session());
    assert_eq!(
        state.auth.last_error.as_deref(),
        Some("Reauthentication required.")
    );
}

#[test]
fn auth_sign_out_preserves_email_for_reentry() {
    let mut state = AppState::default();
    let _ = apply_action(
        &mut state,
        AppAction::AuthChallengeAccepted {
            email: "dev@example.com".to_string(),
            challenge_id: "challenge_abc".to_string(),
        },
    );

    let _ = apply_action(&mut state, AppAction::AuthSignedOut);
    assert_eq!(state.auth.status, AuthStatus::SignedOut);
    assert_eq!(state.auth.email.as_deref(), Some("dev@example.com"));
    assert_eq!(state.auth.challenge_id, None);
    assert!(!state.auth.has_tokens());
}
