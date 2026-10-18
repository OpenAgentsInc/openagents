const PROJECT_INVITATION_PREFIX: &str = "https://openagents.com/projects/join/";
const MAX_LINK_BYTES: usize = 2_048;
const MAX_CODE_BYTES: usize = 200;
const CODE_PREFIX: &str = "prjinv_";

/// Text Android can pass to the system share chooser for a project invitation.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ProjectInvitationShare {
    pub title: String,
    pub text: String,
}

/// Return the original link only when it matches the project invitation format.
pub(crate) fn validated_project_invitation_link(link: &str) -> Option<&str> {
    if link.len() > MAX_LINK_BYTES || !link.is_ascii() {
        return None;
    }
    let code = link.strip_prefix(PROJECT_INVITATION_PREFIX)?;
    if code.len() > MAX_CODE_BYTES || !code.starts_with(CODE_PREFIX) {
        return None;
    }
    let token = code.strip_prefix(CODE_PREFIX)?;
    if token.is_empty()
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return None;
    }
    Some(link)
}

pub(crate) fn project_invitation_share(link: &str) -> Option<ProjectInvitationShare> {
    let link = validated_project_invitation_link(link)?;
    Some(ProjectInvitationShare {
        title: "Join my project".into(),
        text: format!("Join my project on OpenAgents: {link}"),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CODE_PREFIX, MAX_CODE_BYTES, PROJECT_INVITATION_PREFIX, project_invitation_share,
        validated_project_invitation_link,
    };

    const VALID: &str = "https://openagents.com/projects/join/prjinv_Az09_-";

    #[test]
    fn invitation_link_validation_accepts_only_the_bounded_canonical_form() {
        assert_eq!(validated_project_invitation_link(VALID), Some(VALID));
        let max_code = format!(
            "{PROJECT_INVITATION_PREFIX}prjinv_{}",
            "a".repeat(MAX_CODE_BYTES - CODE_PREFIX.len())
        );
        assert_eq!(
            validated_project_invitation_link(&max_code),
            Some(max_code.as_str())
        );
        for invalid in [
            "http://openagents.com/projects/join/prjinv_token",
            "https://www.openagents.com/projects/join/prjinv_token",
            "https://openagents.com/projects/join/prjinv_",
            "https://openagents.com/projects/join/prjinv_token?next=/",
            "https://openagents.com/projects/join/prjinv_token#part",
            "https://openagents.com/projects/join/prjinv_token/extra",
            "https://openagents.com/projects/join/prjinv_token%2Fextra",
            "https://openagents.com/projects/join/prjinv_töken",
        ] {
            assert_eq!(
                validated_project_invitation_link(invalid),
                None,
                "{invalid}"
            );
        }
        let long_token = format!(
            "{PROJECT_INVITATION_PREFIX}prjinv_{}",
            "a".repeat(MAX_CODE_BYTES - CODE_PREFIX.len() + 1)
        );
        assert_eq!(validated_project_invitation_link(&long_token), None);
        let long_input = format!("{}{}", VALID, "a".repeat(2_048));
        assert_eq!(validated_project_invitation_link(&long_input), None);
    }

    #[test]
    fn project_invitation_share_keeps_the_link_only_in_chooser_text() {
        let share = project_invitation_share(VALID).unwrap();
        assert_eq!(share.title, "Join my project");
        assert_eq!(
            share.text,
            format!("Join my project on OpenAgents: {VALID}")
        );
    }
}
