//! Why an import or a pose was refused.

use std::fmt;

/// Why a description, a mesh, a source pin, or a joint vector was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// A file could not be read.
    Read { path: String, reason: String },
    /// An input is over one of the import bounds.
    TooLarge {
        what: String,
        limit: u64,
        actual: u64,
    },
    /// The text is not well-formed XML.
    Xml(String),
    /// The file is XML but not a URDF robot.
    NotUrdf(String),
    /// A description feature this importer does not handle.
    Unsupported { feature: String, at: String },
    /// The link and joint tree is malformed.
    Structure(String),
    /// A value is missing or out of its domain.
    Invalid { at: String, reason: String },
    /// A length or an angle that looks like the wrong unit.
    Units { at: String, reason: String },
    /// A mesh path that is absolute, a URL, missing, or outside the asset folder.
    AssetPath { path: String, reason: String },
    /// A mesh file that cannot be decoded.
    Mesh { path: String, reason: String },
    /// A joint vector with the wrong number of values.
    JointCount { expected: usize, got: usize },
    /// A joint value outside the joint's declared limits.
    Limit {
        joint: String,
        value: f64,
        lower: f64,
        upper: f64,
    },
    /// A joint pose that names no movable joint or holds a non-number.
    Pose(String),
    /// The source pin does not match the files beside it.
    Pin(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, reason } => write!(f, "cannot read {path}: {reason}"),
            Self::TooLarge {
                what,
                limit,
                actual,
            } => write!(f, "{what} is {actual}, over the limit of {limit}"),
            Self::Xml(reason) => write!(f, "the description is not well-formed XML: {reason}"),
            Self::NotUrdf(reason) => write!(f, "not a URDF robot description: {reason}"),
            Self::Unsupported { feature, at } => write!(f, "{at}: {feature} is not supported"),
            Self::Structure(reason) => f.write_str(reason),
            Self::Invalid { at, reason } | Self::Units { at, reason } => {
                write!(f, "{at}: {reason}")
            }
            Self::AssetPath { path, reason } => write!(f, "mesh path {path:?} {reason}"),
            Self::Mesh { path, reason } => write!(f, "mesh {path}: {reason}"),
            Self::JointCount { expected, got } => {
                write!(
                    f,
                    "the robot has {expected} movable joints, but the pose gives {got} values"
                )
            }
            Self::Limit {
                joint,
                value,
                lower,
                upper,
            } => write!(
                f,
                "joint {joint} at {value} is outside its limits [{lower}, {upper}]"
            ),
            Self::Pose(reason) => f.write_str(reason),
            Self::Pin(reason) => write!(f, "source pin: {reason}"),
        }
    }
}

impl std::error::Error for Error {}
