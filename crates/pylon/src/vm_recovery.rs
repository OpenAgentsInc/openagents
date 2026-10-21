//! Recovery of one explicitly maintained GCP VM, independent of an agent run.
//!
//! The controller has an instance-scoped service account and obtains short-lived
//! Compute tokens from its own VM's metadata service. It never creates a VM,
//! changes scheduling, or starts an agent-owned or deliberately stopped machine.
use serde_json::Value;

/// One fixed instance, including its numeric identity to fence name reuse.
#[derive(Debug, Clone)]
pub struct Target {
    pub project: String,
    pub zone: String,
    pub name: String,
    pub id: String,
}

impl Target {
    pub fn new(project: &str, zone: &str, name: &str, id: &str) -> Result<Self, String> {
        if [project, zone, name].iter().any(|s| {
            s.is_empty()
                || s.len() > 63
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        }) || id.is_empty()
            || !id.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("Use a project, zone, VM name, and numeric instance ID.".into());
        }
        Ok(Self {
            project: project.into(),
            zone: zone.into(),
            name: name.into(),
            id: id.into(),
        })
    }

    fn url(&self) -> String {
        format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/zones/{}/instances/{}",
            self.project, self.zone, self.name
        )
    }

    /// A maintenance label is an operator choice; a changed ID is never adopted.
    pub fn should_start(&self, vm: &Value) -> Result<bool, String> {
        if vm["id"].as_str() != Some(self.id.as_str()) {
            return Err(
                "The VM's identity changed. Update the recovery target before starting it.".into(),
            );
        }
        let labels = &vm["labels"];
        let maintained = labels["service"] == "psionic-flint"
            && labels["lifecycle"] == "maintained"
            && labels["desired"] == "running"
            && labels.get("run-id").is_none()
            && labels.get("budget").is_none()
            && labels.get("expires-at").is_none();
        let no_expiry = vm["scheduling"].get("maxRunDuration").is_none()
            && vm["scheduling"].get("terminationTime").is_none();
        Ok(maintained && no_expiry && vm["status"] == "TERMINATED")
    }

    /// Inspect once; start only a stopped, explicitly maintained instance.
    /// Never print metadata tokens or API response bodies in errors.
    pub async fn recover(&self, client: &reqwest::Client) -> Result<&'static str, String> {
        let token: Value = client.get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
            .header("Metadata-Flavor", "Google").send().await
            .map_err(|_| "Could not reach the controller's metadata service.")?
            .error_for_status().map_err(|_| "The controller has no Compute credential.")?
            .json().await.map_err(|_| "The controller's Compute credential could not be read.")?;
        let bearer = token["access_token"]
            .as_str()
            .ok_or("The metadata response has no access token.")?;
        let response = client
            .get(self.url())
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|_| "Could not inspect the GPU VM.")?;
        if !response.status().is_success() {
            return Err(format!(
                "Could not inspect the GPU VM (HTTP {}).",
                response.status().as_u16()
            ));
        }
        let vm: Value = response
            .json()
            .await
            .map_err(|_| "The GPU VM's status could not be read.")?;
        if !self.should_start(&vm)? {
            return Ok("unchanged");
        }
        let response = client
            .post(format!("{}/start", self.url()))
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|_| "Could not request a GPU VM restart.")?;
        if !response.status().is_success() {
            return Err(format!(
                "GPU VM restart was refused (HTTP {}).",
                response.status().as_u16()
            ));
        }
        Ok("starting")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn recovery_requires_the_same_id_an_operator_choice_and_no_run_expiry() {
        let target = Target::new("project", "zone-a", "flint", "123").unwrap();
        let stopped = json!({"id":"123","status":"TERMINATED","scheduling":{},"labels":{
            "service":"psionic-flint","lifecycle":"maintained","desired":"running"}});
        assert!(target.should_start(&stopped).unwrap());
        for state in [
            "RUNNING",
            "STAGING",
            "PROVISIONING",
            "STOPPING",
            "REPAIRING",
            "SUSPENDED",
        ] {
            let mut vm = stopped.clone();
            vm["status"] = json!(state);
            assert!(!target.should_start(&vm).unwrap());
        }
        for (key, value) in [
            ("desired", "stopped"),
            ("service", "another-service"),
            ("lifecycle", "temporary"),
            ("run-id", "an-agent"),
            ("budget", "a-budget"),
            ("expires-at", "tomorrow"),
        ] {
            let mut vm = stopped.clone();
            vm["labels"][key] = json!(value);
            assert!(!target.should_start(&vm).unwrap(), "{key}");
        }
        for key in ["maxRunDuration", "terminationTime"] {
            let mut vm = stopped.clone();
            vm["scheduling"][key] = json!("scheduled");
            assert!(!target.should_start(&vm).unwrap());
        }
        let mut replacement = stopped;
        replacement["id"] = json!("456");
        assert!(target.should_start(&replacement).is_err());
        assert!(Target::new("project", "zone", "flint/another", "123").is_err());
    }
}
