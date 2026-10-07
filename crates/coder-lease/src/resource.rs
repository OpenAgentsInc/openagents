//! The resources a lease can name, and their shapes.

use std::fmt;

/// Whether one holder or several share a resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// One holder at a time.
    Exclusive,
    /// Holders share a capacity; each lease declares an amount of it.
    Counted,
}

/// A resource the broker leases.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Resource {
    /// Concurrent heavy builds, counted in slots.
    Build,
    /// The machine-wide memory budget, in GiB.
    Memory,
    /// A disk budget, in GB, admitted only above the free-space floor.
    Disk,
    /// The quiet machine: no builds run while it is held.
    Quiet,
    /// The owner's real screen, which needs an owner grant.
    Screen,
    /// The shared browser.
    Browser,
    /// The local GPU.
    Gpu,
    /// The Unreal editor.
    Unreal,
    /// The Blender binary.
    Blender,
    /// A single-digest artifact, such as an asset pack (`artifact/<name>`).
    Artifact(String),
    /// A GitHub issue claim (`issue/<n>`).
    Issue(u64),
}

impl Resource {
    /// Parses a resource name, such as `build`, `screen`, or `issue/10755`.
    ///
    /// # Errors
    /// A sentence naming the resources when `name` is none of them.
    pub fn parse(name: &str) -> Result<Resource, String> {
        Ok(match name {
            "build" => Resource::Build,
            "memory" => Resource::Memory,
            "disk" => Resource::Disk,
            "quiet" => Resource::Quiet,
            "screen" => Resource::Screen,
            "browser" => Resource::Browser,
            "gpu" => Resource::Gpu,
            "unreal" => Resource::Unreal,
            "blender" => Resource::Blender,
            other => {
                if let Some(artifact) = other.strip_prefix("artifact/") {
                    if !valid_name(artifact) {
                        return Err(format!(
                            "`{other}` is not an artifact name; use letters, digits, `.`, `_`, and `-`"
                        ));
                    }
                    Resource::Artifact(artifact.to_owned())
                } else if let Some(digits) = other.strip_prefix("issue/") {
                    match digits.parse::<u64>() {
                        Ok(number) if number > 0 && number.to_string() == digits => {
                            Resource::Issue(number)
                        }
                        _ => return Err(format!("`{other}` is not an issue number")),
                    }
                } else {
                    return Err(format!(
                        "`{other}` is not a resource; the resources are {}, artifact/NAME, and issue/N",
                        NAMED.join(", ")
                    ));
                }
            }
        })
    }

    /// Whether one holder or several share it.
    #[must_use]
    pub fn shape(&self) -> Shape {
        match self {
            Resource::Build | Resource::Memory | Resource::Disk => Shape::Counted,
            _ => Shape::Exclusive,
        }
    }

    /// The unit a counted resource's amount is in, for messages.
    #[must_use]
    pub fn unit(&self) -> &'static str {
        match self {
            Resource::Build => "slots",
            Resource::Memory => "GiB",
            Resource::Disk => "GB",
            _ => "",
        }
    }
}

/// The resources with fixed names, in the order the help lists them.
pub const NAMED: [&str; 9] = [
    "build", "memory", "disk", "quiet", "screen", "browser", "gpu", "unreal", "blender",
];

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Resource::Build => f.write_str("build"),
            Resource::Memory => f.write_str("memory"),
            Resource::Disk => f.write_str("disk"),
            Resource::Quiet => f.write_str("quiet"),
            Resource::Screen => f.write_str("screen"),
            Resource::Browser => f.write_str("browser"),
            Resource::Gpu => f.write_str("gpu"),
            Resource::Unreal => f.write_str("unreal"),
            Resource::Blender => f.write_str("blender"),
            Resource::Artifact(name) => write!(f, "artifact/{name}"),
            Resource::Issue(number) => write!(f, "issue/{number}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_shapes_hold() {
        for name in NAMED
            .iter()
            .copied()
            .chain(["artifact/everglade-pack", "issue/10755"])
        {
            let resource = Resource::parse(name).unwrap();
            assert_eq!(resource.to_string(), name);
        }
        assert_eq!(Resource::Build.shape(), Shape::Counted);
        assert_eq!(Resource::Disk.shape(), Shape::Counted);
        assert_eq!(Resource::Quiet.shape(), Shape::Exclusive);
        assert_eq!(Resource::Issue(1).shape(), Shape::Exclusive);
        for bad in [
            "cpu",
            "artifact/",
            "artifact/../x",
            "artifact/a b",
            "issue/0",
            "issue/x",
            "issue/+5",
            "issue/007",
        ] {
            assert!(Resource::parse(bad).is_err(), "{bad}");
        }
    }
}
