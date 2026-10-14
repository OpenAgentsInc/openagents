//! An in-memory Compute Engine for tests (oa-boat's own, and crates that
//! run oa-boat on loopback, such as `oa-computers`): each "VM" is a record
//! here and a local shell with its own home under the `Local` runner's
//! root. Never used by the service itself.

use crate::gce::{Compute, GceError, Image, Instance, Result};
use crate::time;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct World {
    pub instances: BTreeMap<String, Instance>,
    pub images: BTreeMap<String, Image>,
    pub clock: i64,
    pub full_zones: Vec<String>,
    /// Machine types no zone has room for.
    pub full_machines: Vec<String>,
    /// The insert is made but its wait fails in transport.
    pub flaky_zones: Vec<String>,
    /// The wait fails in transport and the VM never appears.
    pub lost_zones: Vec<String>,
    /// The `Local` runner's root: when set, a VM made from an image or a
    /// disk snapshot starts with a copy of its source VM's home, so file
    /// persistence through templates and forks is real.
    pub root: Option<std::path::PathBuf>,
    /// Disk snapshot name to the disk (= VM) it was taken from.
    pub snapshots: BTreeMap<String, String>,
    /// Image name to the disk (= VM) it was made from.
    pub image_sources: BTreeMap<String, String>,
    /// How long each instance insert takes, to widen a test's window.
    pub insert_delay: Option<std::time::Duration>,
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    let _ = std::fs::create_dir_all(to);
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    for e in entries.flatten() {
        // The per-boot run dir is tmpfs on a real VM: never on a disk copy.
        if e.file_name() == ".oa-boat" {
            continue;
        }
        let target = to.join(e.file_name());
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            copy_dir(&e.path(), &target);
        } else {
            let _ = std::fs::copy(e.path(), target);
        }
    }
}

#[derive(Clone, Default)]
pub struct Fake(pub Arc<Mutex<World>>);

impl Fake {
    pub fn tick(w: &mut World) -> String {
        w.clock = w.clock.max(time::now()) + 1;
        time::format(w.clock)
    }
}

pub fn labels(v: &Value) -> BTreeMap<String, String> {
    v.as_object()
        .map(|m| {
            m.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

impl Compute for Fake {
    async fn insert_instance(&self, zone: &str, body: Value) -> Result<()> {
        let delay = self.0.lock().unwrap().insert_delay;
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
        let mut w = self.0.lock().unwrap();
        let machine = body["machineType"].as_str().unwrap_or_default();
        if w.full_zones.iter().any(|z| z == zone)
            || w.full_machines
                .iter()
                .any(|m| machine.ends_with(&format!("/{m}")))
        {
            return Err(GceError::new(409, "ZONE_RESOURCE_POOL_EXHAUSTED", "full"));
        }
        if w.lost_zones.iter().any(|z| z == zone) {
            return Err(GceError::new(0, "transport", "unreachable"));
        }
        let flaky = w.flaky_zones.iter().any(|z| z == zone);
        let name = body["name"].as_str().unwrap().to_owned();
        let init = &body["disks"][0]["initializeParams"];
        let last = |v: &Value| {
            v.as_str()
                .and_then(|s| s.rsplit('/').next())
                .map(str::to_owned)
        };
        let source = last(&init["sourceSnapshot"])
            .and_then(|s| w.snapshots.get(&s).cloned())
            .or_else(|| last(&init["sourceImage"]).and_then(|i| w.image_sources.get(&i).cloned()));
        if let (Some(root), Some(src)) = (&w.root, source) {
            copy_dir(
                &root.join(&src).join("home"),
                &root.join(&name).join("home"),
            );
        }
        let now = Self::tick(&mut w);
        let disk_gb = body["disks"][0]["initializeParams"]["diskSizeGb"]
            .as_str()
            .and_then(|s| s.parse().ok());
        let i = Instance {
            name: name.clone(),
            zone: zone.into(),
            status: "RUNNING".into(),
            labels: labels(&body["labels"]),
            label_fingerprint: "f0".into(),
            ip: Some(name.clone()),
            machine: body["machineType"]
                .as_str()
                .unwrap()
                .rsplit('/')
                .next()
                .unwrap()
                .into(),
            creation: Some(now.clone()),
            last_start: Some(now),
            last_stop: None,
            disk: Some(name.clone()),
            disk_gb,
            spot: body["scheduling"]["provisioningModel"] == "SPOT",
        };
        w.instances.insert(name, i);
        if flaky {
            return Err(GceError::new(0, "transport", "unreachable"));
        }
        Ok(())
    }
    async fn get_instance(&self, _zone: &str, name: &str) -> Result<Option<Instance>> {
        Ok(self.0.lock().unwrap().instances.get(name).cloned())
    }
    async fn list_instances(&self, label: &str, value: &str) -> Result<Vec<Instance>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .instances
            .values()
            .filter(|i| i.labels.get(label).map(String::as_str) == Some(value))
            .cloned()
            .collect())
    }
    async fn instance_action(&self, _zone: &str, name: &str, action: &str) -> Result<()> {
        let mut w = self.0.lock().unwrap();
        let now = Self::tick(&mut w);
        match action {
            "delete" => {
                w.instances.remove(name);
            }
            "stop" => {
                let i = w.instances.get_mut(name).unwrap();
                i.status = "TERMINATED".into();
                i.last_stop = Some(now);
            }
            "start" => {
                let i = w.instances.get_mut(name).unwrap();
                i.status = "RUNNING".into();
                i.last_start = Some(now);
            }
            _ => unreachable!(),
        }
        Ok(())
    }
    async fn set_labels(
        &self,
        _zone: &str,
        name: &str,
        labels: &BTreeMap<String, String>,
        fingerprint: &str,
    ) -> Result<()> {
        let mut w = self.0.lock().unwrap();
        let i = w.instances.get_mut(name).unwrap();
        if i.label_fingerprint != fingerprint {
            return Err(GceError::new(412, "conditionNotMet", "stale"));
        }
        i.labels = labels.clone();
        i.label_fingerprint = format!("{}x", i.label_fingerprint);
        Ok(())
    }
    async fn set_ssh_keys(&self, _zone: &str, _name: &str, _value: &str) -> Result<()> {
        Ok(())
    }
    async fn get_image(&self, name: &str) -> Result<Option<Image>> {
        Ok(self.0.lock().unwrap().images.get(name).cloned())
    }
    async fn image_from_family(&self, family: &str) -> Result<Option<Image>> {
        Ok(Some(Image {
            name: format!("{family}-20261010"),
            id: "1".into(),
            status: "READY".into(),
            disk_gb: 200,
            self_link: format!("projects/test/global/images/{family}-20261010"),
            ..Default::default()
        }))
    }
    async fn list_images(&self, label: &str, value: &str) -> Result<Vec<Image>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .images
            .values()
            .filter(|i| i.labels.get(label).map(String::as_str) == Some(value))
            .cloned()
            .collect())
    }
    async fn insert_image(&self, body: Value) -> Result<()> {
        let mut w = self.0.lock().unwrap();
        let now = Self::tick(&mut w);
        let name = body["name"].as_str().unwrap().to_owned();
        if let Some(disk) = body["sourceDisk"]
            .as_str()
            .and_then(|s| s.rsplit('/').next())
        {
            w.image_sources.insert(name.clone(), disk.to_owned());
        }
        let id = format!("{}", w.clock);
        w.images.insert(
            name.clone(),
            Image {
                name: name.clone(),
                id,
                status: "READY".into(),
                labels: labels(&body["labels"]),
                disk_gb: 200,
                archive_bytes: Some(1234),
                creation: Some(now),
                self_link: format!("projects/test/global/images/{name}"),
                family: None,
            },
        );
        Ok(())
    }
    async fn delete_image(&self, name: &str) -> Result<()> {
        self.0.lock().unwrap().images.remove(name);
        Ok(())
    }
    async fn snapshot_disk(&self, _zone: &str, disk: &str, snapshot: &str) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .snapshots
            .insert(snapshot.into(), disk.into());
        Ok(())
    }
    async fn delete_snapshot(&self, _name: &str) -> Result<()> {
        Ok(())
    }
}
