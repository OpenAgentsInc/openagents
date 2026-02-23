use protocol::jobs::sandbox::SandboxStatus;
use protocol::{SandboxRunRequest, SandboxRunResponse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxRunVerification {
    pub passed: bool,
    pub exit_code: i32,
    pub violations: Vec<String>,
}

pub fn verify_sandbox_run(request: &SandboxRunRequest, response: &SandboxRunResponse) -> SandboxRunVerification {
    let mut violations = Vec::new();

    let requested_image = request.sandbox.image_digest.trim();
    if !requested_image.is_empty() && request.sandbox.image_digest != response.env_info.image_digest {
        violations.push(format!(
            "image_digest mismatch: request={} response={}",
            request.sandbox.image_digest, response.env_info.image_digest
        ));
    }

    if response.runs.len() != request.commands.len() {
        violations.push(format!(
            "run count mismatch: request={} response={}",
            request.commands.len(),
            response.runs.len()
        ));
    }

    for (idx, (expected, actual)) in request
        .commands
        .iter()
        .zip(response.runs.iter())
        .enumerate()
    {
        if expected.cmd != actual.cmd {
            violations.push(format!(
                "cmd mismatch at index {idx}: request={} response={}",
                expected.cmd, actual.cmd
            ));
        }
    }

    let mut exit_code = 0;
    for run in &response.runs {
        if let Some(preview) = run.stdout_preview.as_deref() {
            if preview.len() > 16_384 {
                violations.push("stdout_preview exceeds 16KiB cap".to_string());
            }
        }
        if let Some(preview) = run.stderr_preview.as_deref() {
            if preview.len() > 16_384 {
                violations.push("stderr_preview exceeds 16KiB cap".to_string());
            }
        }
        if run.exit_code != 0 {
            exit_code = run.exit_code;
            break;
        }
    }

    if response.artifacts.len() > 128 {
        violations.push("artifact count exceeds 128 cap".to_string());
    }
    for artifact in &response.artifacts {
        if artifact.path.len() > 1_024 {
            violations.push("artifact path exceeds 1024 byte cap".to_string());
        }
        if artifact.sha256.len() > 128 {
            violations.push("artifact sha256 exceeds 128 byte cap".to_string());
        }
    }

    match response.status {
        SandboxStatus::Success => {
            if exit_code != 0 {
                violations.push("status success but a command exit_code was non-zero".to_string());
            }
            if response.error.is_some() {
                violations.push("status success but error field was present".to_string());
            }
        }
        SandboxStatus::Failed => {
            // Failed is a valid job outcome; only flag provider-level inconsistencies.
            if exit_code == 0 {
                violations.push("status failed but no failing exit_code reported".to_string());
                exit_code = 1;
            }
            if response.error.is_some() {
                violations.push("error message present but status was not error".to_string());
            }
        }
        SandboxStatus::Timeout | SandboxStatus::Cancelled => {
            violations.push(format!("provider status {:?}", response.status));
            if exit_code == 0 {
                exit_code = 1;
            }
            if response.error.is_some() {
                violations.push("error message present but status was not error".to_string());
            }
        }
        SandboxStatus::Error => {
            violations.push("provider status error".to_string());
            if exit_code == 0 {
                exit_code = 1;
            }
            if response
                .error
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                violations.push("status error but error message missing".to_string());
            }
        }
    };

    let passed = violations.is_empty() && response.status == SandboxStatus::Success && exit_code == 0;
    if !passed && exit_code == 0 {
        exit_code = 1;
    }

    SandboxRunVerification {
        passed,
        exit_code,
        violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::jobs::sandbox::{CommandResult, EnvInfo};
    use protocol::provenance::Provenance;

    fn request() -> SandboxRunRequest {
        SandboxRunRequest {
            sandbox: protocol::jobs::sandbox::SandboxConfig {
                image_digest: "sha256:test".to_string(),
                ..Default::default()
            },
            commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
            ..Default::default()
        }
    }

    fn response_success() -> SandboxRunResponse {
        SandboxRunResponse {
            env_info: EnvInfo {
                image_digest: "sha256:test".to_string(),
                hostname: None,
                system_info: None,
            },
            runs: vec![CommandResult {
                cmd: "echo hi".to_string(),
                exit_code: 0,
                duration_ms: 1,
                stdout_sha256: "stdout".to_string(),
                stderr_sha256: "stderr".to_string(),
                stdout_preview: None,
                stderr_preview: None,
            }],
            artifacts: Vec::new(),
            status: SandboxStatus::Success,
            error: None,
            provenance: Provenance::new("test"),
        }
    }

    #[test]
    fn sandbox_verification_passes_on_success() {
        let outcome = verify_sandbox_run(&request(), &response_success());
        assert!(outcome.passed);
        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.violations.is_empty());
    }

    #[test]
    fn sandbox_verification_fails_on_nonzero_exit() {
        let mut response = response_success();
        response.runs[0].exit_code = 2;
        response.status = SandboxStatus::Failed;
        let outcome = verify_sandbox_run(&request(), &response);
        assert!(!outcome.passed);
        assert_eq!(outcome.exit_code, 2);
        assert!(outcome.violations.is_empty());
    }

    #[test]
    fn sandbox_verification_flags_timeout_as_provider_violation() {
        let mut response = response_success();
        response.status = SandboxStatus::Timeout;
        let outcome = verify_sandbox_run(&request(), &response);
        assert!(!outcome.passed);
        assert!(!outcome.violations.is_empty());
    }
}
