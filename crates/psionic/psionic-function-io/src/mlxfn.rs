use std::{collections::BTreeMap, fs, path::Path};

use psionic_core::{
    DType, Device, PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope, Shape, Tensor,
    TensorData, TensorId,
};
use psionic_ir::{GraphBuilder, OpKind};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{FunctionArtifact, FunctionArtifactDirection, FunctionIoError};

const MLXFN_EXPORT_VERSION: &str = "0.31.1";
const SUPPORTED_MLXFN_VERSIONS: &[&str] = &["0.31.0", "0.31.1"];
const MLX_DEVICE_CPU: i32 = 0;
const MLX_REDUCE_SUM: i32 = 2;

type SliceVectors = (Vec<usize>, Vec<usize>, Vec<usize>);

/// Artifact family for bounded `.mlxfn` compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MlxfnCompatibilityFormat {
    /// One bounded MLX `.mlxfn` artifact.
    Mlxfn,
}

impl MlxfnCompatibilityFormat {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mlxfn => "mlxfn",
        }
    }
}

/// Stable receipt emitted for one bounded `.mlxfn` import or export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MlxfnCompatibilityReceipt {
    /// Artifact family that crossed the boundary.
    pub format: MlxfnCompatibilityFormat,
    /// Whether the boundary was import or export.
    pub direction: FunctionArtifactDirection,
    /// Stable SHA-256 digest over the artifact bytes.
    pub artifact_sha256: String,
    /// Exact artifact length in bytes.
    pub artifact_bytes: usize,
    /// MLX version string carried in the artifact header.
    pub mlx_version: String,
    /// Number of traces carried in the artifact.
    pub function_count: u32,
    /// Whether the artifact advertises shapeless behavior.
    pub shapeless: bool,
    /// Keyword input names carried by the trace.
    pub kwarg_keys: Vec<String>,
    /// Primitive names carried by the trace in tape order.
    pub primitive_names: Vec<String>,
    /// Entry point assigned on the Psionic side.
    pub entrypoint: String,
    /// Stable digest of the native Psionic function artifact.
    pub function_artifact_digest: String,
    /// Whether an attached native compile bundle could not be represented and
    /// was therefore omitted from the `.mlxfn` artifact.
    pub stripped_compile_bundle: bool,
}

impl MlxfnCompatibilityReceipt {
    fn new(
        direction: FunctionArtifactDirection,
        bytes: &[u8],
        artifact: &FunctionArtifact,
        header: &MlxfnHeader,
        kwarg_keys: Vec<String>,
        primitive_names: Vec<String>,
        stripped_compile_bundle: bool,
    ) -> Self {
        Self {
            format: MlxfnCompatibilityFormat::Mlxfn,
            direction,
            artifact_sha256: hex::encode(Sha256::digest(bytes)),
            artifact_bytes: bytes.len(),
            mlx_version: header.mlx_version.clone(),
            function_count: header.function_count as u32,
            shapeless: header.shapeless,
            kwarg_keys,
            primitive_names,
            entrypoint: artifact.entrypoint().to_string(),
            function_artifact_digest: artifact.stable_digest(),
            stripped_compile_bundle,
        }
    }
}

/// Error returned while converting to or from one bounded `.mlxfn` artifact.
#[derive(Debug, Error)]
pub enum MlxfnIoError {
    /// One native Psionic function-artifact operation failed.
    #[error(transparent)]
    Function(#[from] FunctionIoError),
    /// One `.mlxfn` header carried a version outside the bounded compatibility window.
    #[error(
        "bounded `.mlxfn` compatibility supports only MLX versions 0.31.0 through 0.31.1, found `{version}`"
    )]
    UnsupportedVersion {
        /// Unsupported MLX version string.
        version: String,
    },
    /// The artifact carried more than one function trace.
    #[error("bounded `.mlxfn` compatibility supports exactly one trace, found {count}")]
    UnsupportedFunctionCount {
        /// Number of traces declared by the artifact.
        count: i32,
    },
    /// The artifact used keyword inputs.
    #[error(
        "bounded `.mlxfn` compatibility supports positional-only traces, found keyword inputs {keys:?}"
    )]
    UnsupportedKwargKeys {
        /// Keyword keys declared by the artifact.
        keys: Vec<String>,
    },
    /// The artifact carried shapeless behavior.
    #[error("bounded `.mlxfn` compatibility does not support shapeless traces")]
    UnsupportedShapeless,
    /// The artifact carried a primitive outside the bounded current subset.
    #[error("bounded `.mlxfn` compatibility does not support primitive `{primitive}`")]
    UnsupportedPrimitive {
        /// Unsupported primitive label.
        primitive: String,
    },
    /// The artifact carried one multi-output primitive.
    #[error(
        "bounded `.mlxfn` compatibility does not support multi-output primitive `{primitive}` with {siblings} siblings"
    )]
    UnsupportedMultiOutputPrimitive {
        /// Primitive label.
        primitive: String,
        /// Sibling count declared by the artifact.
        siblings: u64,
    },
    /// The artifact carried one unsupported device type.
    #[error(
        "bounded `.mlxfn` compatibility supports only CPU traces, found device_type {device_type} on stream {stream_index}"
    )]
    UnsupportedDevice {
        /// Unsupported MLX device type.
        device_type: i32,
        /// Stream index attached to the primitive.
        stream_index: i32,
    },
    /// One native Psionic dtype cannot be represented honestly inside the bounded `.mlxfn` subset.
    #[error("bounded `.mlxfn` compatibility cannot export dtype {dtype:?}: {detail}")]
    UnsupportedExportDType {
        /// Dtype that could not be exported.
        dtype: DType,
        /// Plain-language detail.
        detail: String,
    },
    /// One MLX dtype could not be imported into the bounded Psionic subset.
    #[error(
        "bounded `.mlxfn` compatibility cannot import MLX dtype value {dtype_value} size {dtype_size}: {detail}"
    )]
    UnsupportedImportDType {
        /// Raw MLX dtype tag.
        dtype_value: i32,
        /// Raw MLX dtype size.
        dtype_size: u8,
        /// Plain-language detail.
        detail: String,
    },
    /// One constant used a dtype outside the bounded current subset.
    #[error("bounded `.mlxfn` compatibility supports only dense f32 constants, found {dtype:?}")]
    UnsupportedConstantDType {
        /// Dtype declared by the constant.
        dtype: DType,
    },
    /// One `.mlxfn` artifact was malformed.
    #[error("invalid `.mlxfn` artifact: {detail}")]
    InvalidArtifact {
        /// Plain-language validation detail.
        detail: String,
    },
    /// One filesystem operation failed.
    #[error("path `{path}` failed during {operation}: {message}")]
    Io {
        /// Path that failed.
        path: String,
        /// Operation label.
        operation: &'static str,
        /// Plain-language error.
        message: String,
    },
}

impl MlxfnIoError {
    /// Returns the canonical refusal when the compatibility failure belongs to
    /// one explicit artifact, layout, or compatibility boundary.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::Function(error) => error.refusal(),
            Self::UnsupportedVersion { .. }
            | Self::UnsupportedFunctionCount { .. }
            | Self::UnsupportedImportDType { .. }
            | Self::InvalidArtifact { .. } => Some(PsionicRefusal::new(
                PsionicRefusalCode::SerializationIncompatibility,
                PsionicRefusalScope::Graph,
                self.to_string(),
            )),
            Self::UnsupportedKwargKeys { .. } | Self::UnsupportedPrimitive { .. } => {
                Some(PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    self.to_string(),
                ))
            }
            Self::UnsupportedShapeless | Self::UnsupportedMultiOutputPrimitive { .. } => {
                Some(PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedLayout,
                    PsionicRefusalScope::Graph,
                    self.to_string(),
                ))
            }
            Self::UnsupportedDevice { .. } => Some(PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedBackendCapability,
                PsionicRefusalScope::Graph,
                self.to_string(),
            )),
            Self::UnsupportedExportDType { .. } | Self::UnsupportedConstantDType { .. } => {
                Some(PsionicRefusal::new(
                    PsionicRefusalCode::SerializationIncompatibility,
                    PsionicRefusalScope::Graph,
                    self.to_string(),
                ))
            }
            Self::Io { .. } => None,
        }
    }
}

/// Encodes one native function artifact into one bounded `.mlxfn` artifact.
pub fn encode_mlxfn_function_artifact(
    artifact: &FunctionArtifact,
) -> Result<(Vec<u8>, MlxfnCompatibilityReceipt), MlxfnIoError> {
    artifact.validate()?;
    let stripped_compile_bundle = artifact.compile_bundle.is_some();
    let trace = MlxfnTrace::from_function_artifact(artifact)?;
    let header = MlxfnHeader {
        mlx_version: String::from(MLXFN_EXPORT_VERSION),
        function_count: 1,
        shapeless: false,
    };
    let mut writer = MlxfnWriter::default();
    writer.write_header(&header);
    writer.write_trace(&trace)?;
    let bytes = writer.finish();
    let receipt = MlxfnCompatibilityReceipt::new(
        FunctionArtifactDirection::Export,
        &bytes,
        artifact,
        &header,
        trace.kwarg_keys.clone(),
        trace.primitive_names(),
        stripped_compile_bundle,
    );
    Ok((bytes, receipt))
}

/// Decodes one bounded `.mlxfn` artifact into one native Psionic function artifact.
pub fn decode_mlxfn_function_artifact(
    bytes: &[u8],
    entrypoint: impl Into<String>,
) -> Result<(FunctionArtifact, MlxfnCompatibilityReceipt), MlxfnIoError> {
    let entrypoint = entrypoint.into();
    let mut reader = MlxfnReader::new(bytes);
    let header = reader.read_header()?;
    if !SUPPORTED_MLXFN_VERSIONS.contains(&header.mlx_version.as_str()) {
        return Err(MlxfnIoError::UnsupportedVersion {
            version: header.mlx_version,
        });
    }
    if header.function_count != 1 {
        return Err(MlxfnIoError::UnsupportedFunctionCount {
            count: header.function_count,
        });
    }
    if header.shapeless {
        return Err(MlxfnIoError::UnsupportedShapeless);
    }
    let trace = reader.read_trace()?;
    if !trace.kwarg_keys.is_empty() {
        return Err(MlxfnIoError::UnsupportedKwargKeys {
            keys: trace.kwarg_keys,
        });
    }
    let primitive_names = trace.primitive_names();
    let artifact = trace.into_function_artifact(entrypoint)?;
    reader.finish()?;
    let receipt = MlxfnCompatibilityReceipt::new(
        FunctionArtifactDirection::Import,
        bytes,
        &artifact,
        &header,
        Vec::new(),
        primitive_names,
        false,
    );
    Ok((artifact, receipt))
}

/// Saves one bounded `.mlxfn` artifact to the provided path.
pub fn save_mlxfn_function_artifact_path(
    artifact: &FunctionArtifact,
    path: impl AsRef<Path>,
) -> Result<MlxfnCompatibilityReceipt, MlxfnIoError> {
    let path = path.as_ref();
    let (bytes, receipt) = encode_mlxfn_function_artifact(artifact)?;
    fs::write(path, bytes).map_err(|error| MlxfnIoError::Io {
        path: path.display().to_string(),
        operation: "write",
        message: error.to_string(),
    })?;
    Ok(receipt)
}

/// Loads one bounded `.mlxfn` artifact from the provided path.
pub fn load_mlxfn_function_artifact_path(
    path: impl AsRef<Path>,
    entrypoint: impl Into<String>,
) -> Result<(FunctionArtifact, MlxfnCompatibilityReceipt), MlxfnIoError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| MlxfnIoError::Io {
        path: path.display().to_string(),
        operation: "read",
        message: error.to_string(),
    })?;
    decode_mlxfn_function_artifact(&bytes, entrypoint)
}

#[derive(Clone, Debug)]
struct MlxfnHeader {
    mlx_version: String,
    function_count: i32,
    shapeless: bool,
}

#[derive(Clone, Debug)]
struct MlxfnTensorSpec {
    shape: Shape,
    dtype: DType,
}

#[derive(Clone, Debug)]
enum MlxfnTapeEntry {
    Input {
        id: u64,
    },
    Constant {
        id: u64,
        spec: MlxfnTensorSpec,
        values: Vec<f32>,
    },
    Primitive {
        id: u64,
        stream_index: i32,
        input_ids: Vec<u64>,
        primitive: MlxfnPrimitive,
        spec: MlxfnTensorSpec,
    },
}

#[derive(Clone, Debug)]
enum MlxfnPrimitive {
    Add,
    Multiply,
    Matmul,
    StopGradient,
    AsType(DType),
    Reshape(Shape),
    Transpose(Vec<usize>),
    Concatenate(usize),
    Broadcast(Shape),
    ReduceSum(Vec<usize>),
    Slice {
        start: Vec<usize>,
        end: Vec<usize>,
        strides: Vec<usize>,
    },
}

impl MlxfnPrimitive {
    fn name(&self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Multiply => "Multiply",
            Self::Matmul => "Matmul",
            Self::StopGradient => "StopGradient",
            Self::AsType(_) => "AsType",
            Self::Reshape(_) => "Reshape",
            Self::Transpose(_) => "Transpose",
            Self::Concatenate(_) => "Concatenate",
            Self::Broadcast(_) => "Broadcast",
            Self::ReduceSum(_) => "Sum",
            Self::Slice { .. } => "Slice",
        }
    }
}

#[derive(Clone, Debug)]
struct MlxfnTrace {
    kwarg_keys: Vec<String>,
    trace_input_ids: Vec<u64>,
    trace_inputs: Vec<MlxfnTensorSpec>,
    trace_output_ids: Vec<u64>,
    tape: Vec<MlxfnTapeEntry>,
}

impl MlxfnTrace {
    fn from_function_artifact(artifact: &FunctionArtifact) -> Result<Self, MlxfnIoError> {
        let mut tape = Vec::with_capacity(artifact.graph.nodes().len());
        let trace_input_ids = artifact
            .export_contract
            .input_bindings
            .iter()
            .map(|binding| u64::from(binding.tensor.0))
            .collect::<Vec<_>>();
        let trace_inputs = artifact
            .export_contract
            .input_bindings
            .iter()
            .map(|binding| MlxfnTensorSpec {
                shape: binding.spec.shape().clone(),
                dtype: binding.spec.dtype(),
            })
            .collect::<Vec<_>>();
        let trace_output_ids = artifact
            .export_contract
            .output_bindings
            .iter()
            .map(|binding| u64::from(binding.tensor.0))
            .collect::<Vec<_>>();

        for node in artifact.graph.nodes() {
            let id = u64::from(node.tensor().id().0);
            let spec = MlxfnTensorSpec {
                shape: node.tensor().spec().shape().clone(),
                dtype: node.tensor().spec().dtype(),
            };
            let entry = match node.op() {
                OpKind::Input { .. } => MlxfnTapeEntry::Input { id },
                OpKind::Constant { data } => match data {
                    TensorData::F32(values) => MlxfnTapeEntry::Constant {
                        id,
                        spec,
                        values: values.clone(),
                    },
                    TensorData::QuantizedBlocks(_) => {
                        return Err(MlxfnIoError::UnsupportedConstantDType {
                            dtype: node.tensor().spec().dtype(),
                        });
                    }
                },
                OpKind::Detach => MlxfnTapeEntry::Primitive {
                    id,
                    stream_index: 0,
                    input_ids: tensor_ids_to_u64(node.inputs()),
                    primitive: MlxfnPrimitive::StopGradient,
                    spec,
                },
                OpKind::Add => MlxfnTapeEntry::Primitive {
                    id,
                    stream_index: 0,
                    input_ids: tensor_ids_to_u64(node.inputs()),
                    primitive: MlxfnPrimitive::Add,
                    spec,
                },
                OpKind::Mul => MlxfnTapeEntry::Primitive {
                    id,
                    stream_index: 0,
                    input_ids: tensor_ids_to_u64(node.inputs()),
                    primitive: MlxfnPrimitive::Multiply,
                    spec,
                },
                OpKind::Matmul => MlxfnTapeEntry::Primitive {
                    id,
                    stream_index: 0,
                    input_ids: tensor_ids_to_u64(node.inputs()),
                    primitive: MlxfnPrimitive::Matmul,
                    spec,
                },
                OpKind::Reshape => MlxfnTapeEntry::Primitive {
                    id,
                    stream_index: 0,
                    input_ids: tensor_ids_to_u64(node.inputs()),
                    primitive: MlxfnPrimitive::Reshape(node.tensor().spec().shape().clone()),
                    spec,
                },
                OpKind::Permute { axes } => MlxfnTapeEntry::Primitive {
                    id,
                    stream_index: 0,
                    input_ids: tensor_ids_to_u64(node.inputs()),
                    primitive: MlxfnPrimitive::Transpose(axes.clone()),
                    spec,
                },
                OpKind::Slice { axis, start, end } => {
                    let input_shape = input_shape(artifact, node.inputs().first().copied())?;
                    let (start_indices, end_indices, strides) =
                        mlxfn_slice_vectors(&input_shape, *axis, *start, *end)?;
                    MlxfnTapeEntry::Primitive {
                        id,
                        stream_index: 0,
                        input_ids: tensor_ids_to_u64(node.inputs()),
                        primitive: MlxfnPrimitive::Slice {
                            start: start_indices,
                            end: end_indices,
                            strides,
                        },
                        spec,
                    }
                }
                OpKind::Select { .. } => {
                    return Err(MlxfnIoError::UnsupportedPrimitive {
                        primitive: String::from("Select"),
                    });
                }
                OpKind::Concat { axis } => MlxfnTapeEntry::Primitive {
                    id,
                    stream_index: 0,
                    input_ids: tensor_ids_to_u64(node.inputs()),
                    primitive: MlxfnPrimitive::Concatenate(*axis),
                    spec,
                },
                OpKind::Expand { shape } => MlxfnTapeEntry::Primitive {
                    id,
                    stream_index: 0,
                    input_ids: tensor_ids_to_u64(node.inputs()),
                    primitive: MlxfnPrimitive::Broadcast(shape.clone()),
                    spec,
                },
                OpKind::Cast { dtype } => MlxfnTapeEntry::Primitive {
                    id,
                    stream_index: 0,
                    input_ids: tensor_ids_to_u64(node.inputs()),
                    primitive: MlxfnPrimitive::AsType(*dtype),
                    spec,
                },
                OpKind::ReduceSum { axis } => {
                    let input_shape = input_shape(artifact, node.inputs().first().copied())?;
                    let axes = axis.map_or_else(
                        || (0..input_shape.rank()).collect::<Vec<_>>(),
                        |axis| vec![axis],
                    );
                    MlxfnTapeEntry::Primitive {
                        id,
                        stream_index: 0,
                        input_ids: tensor_ids_to_u64(node.inputs()),
                        primitive: MlxfnPrimitive::ReduceSum(axes),
                        spec,
                    }
                }
                OpKind::BackendExtension { op } => {
                    return Err(MlxfnIoError::UnsupportedPrimitive {
                        primitive: op.label().to_string(),
                    });
                }
            };
            tape.push(entry);
        }

        Ok(Self {
            kwarg_keys: Vec::new(),
            trace_input_ids,
            trace_inputs,
            trace_output_ids,
            tape,
        })
    }

    fn into_function_artifact(self, entrypoint: String) -> Result<FunctionArtifact, MlxfnIoError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let mut tensors = BTreeMap::<u64, Tensor>::new();

        if self.trace_input_ids.len() != self.trace_inputs.len() {
            return Err(MlxfnIoError::InvalidArtifact {
                detail: String::from("trace input ids and trace input specs length mismatch"),
            });
        }

        for (index, (id, spec)) in self
            .trace_input_ids
            .iter()
            .copied()
            .zip(self.trace_inputs.iter())
            .enumerate()
        {
            let tensor = builder.input(format!("input_{index}"), spec.shape.clone(), spec.dtype);
            tensors.insert(id, tensor);
        }

        for entry in self.tape {
            match entry {
                MlxfnTapeEntry::Input { id } => {
                    if !tensors.contains_key(&id) {
                        return Err(MlxfnIoError::InvalidArtifact {
                            detail: format!(
                                "input tape entry `{id}` did not match any trace input"
                            ),
                        });
                    }
                }
                MlxfnTapeEntry::Constant { id, spec, values } => {
                    let tensor =
                        builder
                            .constant_f32(spec.shape.clone(), values)
                            .map_err(|error| MlxfnIoError::InvalidArtifact {
                                detail: error.to_string(),
                            })?;
                    ensure_tensor_matches_spec(&tensor, &spec)?;
                    tensors.insert(id, tensor);
                }
                MlxfnTapeEntry::Primitive {
                    id,
                    input_ids,
                    primitive,
                    spec,
                    ..
                } => {
                    let inputs = input_ids
                        .iter()
                        .map(|input_id| {
                            tensors
                                .get(input_id)
                                .cloned()
                                .ok_or_else(|| MlxfnIoError::InvalidArtifact {
                                    detail: format!(
                                        "primitive output `{id}` referenced unknown input `{input_id}`"
                                    ),
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let tensor = match primitive {
                        MlxfnPrimitive::Add => {
                            builder.add(&inputs[0], &inputs[1]).map_err(|error| {
                                MlxfnIoError::InvalidArtifact {
                                    detail: error.to_string(),
                                }
                            })?
                        }
                        MlxfnPrimitive::Multiply => {
                            builder.mul(&inputs[0], &inputs[1]).map_err(|error| {
                                MlxfnIoError::InvalidArtifact {
                                    detail: error.to_string(),
                                }
                            })?
                        }
                        MlxfnPrimitive::Matmul => {
                            builder.matmul(&inputs[0], &inputs[1]).map_err(|error| {
                                MlxfnIoError::InvalidArtifact {
                                    detail: error.to_string(),
                                }
                            })?
                        }
                        MlxfnPrimitive::StopGradient => builder.detach(&inputs[0]),
                        MlxfnPrimitive::AsType(dtype) => {
                            builder.cast(&inputs[0], dtype).map_err(|error| {
                                MlxfnIoError::InvalidArtifact {
                                    detail: error.to_string(),
                                }
                            })?
                        }
                        MlxfnPrimitive::Reshape(shape) => builder
                            .reshape(&inputs[0], shape)
                            .map_err(|error| MlxfnIoError::InvalidArtifact {
                                detail: error.to_string(),
                            })?,
                        MlxfnPrimitive::Transpose(axes) => builder
                            .permute(&inputs[0], axes)
                            .map_err(|error| MlxfnIoError::InvalidArtifact {
                                detail: error.to_string(),
                            })?,
                        MlxfnPrimitive::Concatenate(axis) => builder
                            .concat(&inputs, axis)
                            .map_err(|error| MlxfnIoError::InvalidArtifact {
                                detail: error.to_string(),
                            })?,
                        MlxfnPrimitive::Broadcast(shape) => builder
                            .expand(&inputs[0], shape)
                            .map_err(|error| MlxfnIoError::InvalidArtifact {
                                detail: error.to_string(),
                            })?,
                        MlxfnPrimitive::ReduceSum(axes) => {
                            if axes.len() == 1 {
                                builder
                                    .reduce_sum_axis(&inputs[0], axes[0])
                                    .map_err(|error| MlxfnIoError::InvalidArtifact {
                                        detail: error.to_string(),
                                    })?
                            } else {
                                let expected_axes =
                                    (0..inputs[0].spec().shape().rank()).collect::<Vec<_>>();
                                if axes != expected_axes {
                                    return Err(MlxfnIoError::UnsupportedPrimitive {
                                        primitive: String::from(
                                            "Sum with non-single-axis and non-all-axis reduction",
                                        ),
                                    });
                                }
                                builder.reduce_sum(&inputs[0])
                            }
                        }
                        MlxfnPrimitive::Slice {
                            start,
                            end,
                            strides,
                        } => {
                            let (axis, start, end) = decode_single_axis_slice(
                                inputs[0].spec().shape(),
                                &start,
                                &end,
                                &strides,
                            )?;
                            builder
                                .slice(&inputs[0], axis, start, end)
                                .map_err(|error| MlxfnIoError::InvalidArtifact {
                                    detail: error.to_string(),
                                })?
                        }
                    };
                    ensure_tensor_matches_spec(&tensor, &spec)?;
                    tensors.insert(id, tensor);
                }
            }
        }

        let outputs = self
            .trace_output_ids
            .iter()
            .map(|id| {
                tensors
                    .get(id)
                    .cloned()
                    .ok_or_else(|| MlxfnIoError::InvalidArtifact {
                        detail: format!("trace output id `{id}` was not produced by the tape"),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let graph = builder.finish(outputs);
        Ok(FunctionArtifact::from_graph(&graph, entrypoint)?)
    }

    fn primitive_names(&self) -> Vec<String> {
        self.tape
            .iter()
            .filter_map(|entry| match entry {
                MlxfnTapeEntry::Primitive { primitive, .. } => Some(String::from(primitive.name())),
                _ => None,
            })
            .collect()
    }
}

#[derive(Default)]
struct MlxfnWriter {
    bytes: Vec<u8>,
}

impl MlxfnWriter {
    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn write_header(&mut self, header: &MlxfnHeader) {
        self.write_string(header.mlx_version.as_str());
        self.write_i32(header.function_count);
        self.write_bool(header.shapeless);
    }

    fn write_trace(&mut self, trace: &MlxfnTrace) -> Result<(), MlxfnIoError> {
        self.write_vec(&trace.kwarg_keys, |writer, key| writer.write_string(key));
        self.write_vec(&trace.trace_input_ids, |writer, id| writer.write_u64(*id));
        self.write_try_vec(&trace.trace_inputs, |writer, spec| {
            writer.write_tensor_spec(spec)
        })?;
        self.write_vec(&trace.trace_output_ids, |writer, id| writer.write_u64(*id));
        self.write_u64(trace.tape.len() as u64);
        for entry in &trace.tape {
            match entry {
                MlxfnTapeEntry::Input { id } => {
                    self.write_u64(*id);
                    self.write_bool(false);
                    self.write_bool(false);
                }
                MlxfnTapeEntry::Constant { id, spec, values } => {
                    if spec.dtype != DType::F32 {
                        return Err(MlxfnIoError::UnsupportedConstantDType { dtype: spec.dtype });
                    }
                    self.write_u64(*id);
                    self.write_bool(false);
                    self.write_bool(true);
                    self.write_tensor_spec(spec)?;
                    for value in values {
                        self.write_f32(*value);
                    }
                }
                MlxfnTapeEntry::Primitive {
                    id,
                    stream_index,
                    input_ids,
                    primitive,
                    spec,
                } => {
                    self.write_u64(*id);
                    self.write_bool(true);
                    self.write_vec(input_ids, |writer, input_id| writer.write_u64(*input_id));
                    self.write_stream(*stream_index, MLX_DEVICE_CPU, 0);
                    self.write_string(primitive.name());
                    self.write_primitive_state(primitive)?;
                    self.write_u64(0);
                    self.write_shape(&spec.shape)?;
                    self.write_dtype(spec.dtype)?;
                }
            }
        }
        Ok(())
    }

    fn write_stream(&mut self, stream_index: i32, device_type: i32, device_index: i32) {
        self.write_i32(stream_index);
        self.write_i32(device_type);
        self.write_i32(device_index);
    }

    fn write_primitive_state(&mut self, primitive: &MlxfnPrimitive) -> Result<(), MlxfnIoError> {
        match primitive {
            MlxfnPrimitive::Add
            | MlxfnPrimitive::Multiply
            | MlxfnPrimitive::Matmul
            | MlxfnPrimitive::StopGradient => Ok(()),
            MlxfnPrimitive::AsType(dtype) => self.write_dtype(*dtype),
            MlxfnPrimitive::Reshape(shape) | MlxfnPrimitive::Broadcast(shape) => {
                self.write_shape(shape)
            }
            MlxfnPrimitive::Transpose(axes) => self.write_usize_vec_as_i32(axes),
            MlxfnPrimitive::Concatenate(axis) => {
                self.write_i32(to_i32(*axis, "concat axis")?);
                Ok(())
            }
            MlxfnPrimitive::ReduceSum(axes) => {
                self.write_i32(MLX_REDUCE_SUM);
                self.write_usize_vec_as_i32(axes)
            }
            MlxfnPrimitive::Slice {
                start,
                end,
                strides,
            } => {
                self.write_usize_vec_as_i32(start)?;
                self.write_usize_vec_as_i32(end)?;
                self.write_usize_vec_as_i32(strides)
            }
        }
    }

    fn write_tensor_spec(&mut self, spec: &MlxfnTensorSpec) -> Result<(), MlxfnIoError> {
        self.write_shape(&spec.shape)?;
        self.write_dtype(spec.dtype)
    }

    fn write_dtype(&mut self, dtype: DType) -> Result<(), MlxfnIoError> {
        let (value, size) = mlx_dtype_from_psionic(dtype)?;
        self.write_i32(value);
        self.write_u8(size);
        Ok(())
    }

    fn write_shape(&mut self, shape: &Shape) -> Result<(), MlxfnIoError> {
        self.write_u64(shape.rank() as u64);
        for dim in shape.dims() {
            self.write_i32(to_i32(*dim, "shape dimension")?);
        }
        Ok(())
    }

    fn write_usize_vec_as_i32(&mut self, values: &[usize]) -> Result<(), MlxfnIoError> {
        self.write_u64(values.len() as u64);
        for value in values {
            self.write_i32(to_i32(*value, "vector element")?);
        }
        Ok(())
    }

    fn write_vec<T, F>(&mut self, values: &[T], mut write_one: F)
    where
        F: FnMut(&mut Self, &T),
    {
        self.write_u64(values.len() as u64);
        for value in values {
            write_one(self, value);
        }
    }

    fn write_try_vec<T, F>(&mut self, values: &[T], mut write_one: F) -> Result<(), MlxfnIoError>
    where
        F: FnMut(&mut Self, &T) -> Result<(), MlxfnIoError>,
    {
        self.write_u64(values.len() as u64);
        for value in values {
            write_one(self, value)?;
        }
        Ok(())
    }

    fn write_string(&mut self, value: &str) {
        self.write_u64(value.len() as u64);
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn write_bool(&mut self, value: bool) {
        self.write_u8(u8::from(value));
    }

    fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn write_i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
}

struct MlxfnReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> MlxfnReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn finish(&self) -> Result<(), MlxfnIoError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(MlxfnIoError::InvalidArtifact {
                detail: format!(
                    "artifact contained {} trailing bytes after the declared trace",
                    self.bytes.len() - self.offset
                ),
            })
        }
    }

    fn read_header(&mut self) -> Result<MlxfnHeader, MlxfnIoError> {
        Ok(MlxfnHeader {
            mlx_version: self.read_string()?,
            function_count: self.read_i32()?,
            shapeless: self.read_bool()?,
        })
    }

    fn read_trace(&mut self) -> Result<MlxfnTrace, MlxfnIoError> {
        let kwarg_keys = self.read_vec(|reader| reader.read_string())?;
        let trace_input_ids = self.read_vec(|reader| reader.read_u64())?;
        let trace_inputs = self.read_vec(|reader| reader.read_tensor_spec())?;
        let trace_output_ids = self.read_vec(|reader| reader.read_u64())?;
        let tape_size = self.read_u64()?;
        let mut tape = Vec::with_capacity(tape_size as usize);
        for _ in 0..tape_size {
            let id = self.read_u64()?;
            let has_primitive = self.read_bool()?;
            if has_primitive {
                let input_ids = self.read_vec(|reader| reader.read_u64())?;
                let (stream_index, device_type) = self.read_stream()?;
                if device_type != MLX_DEVICE_CPU {
                    return Err(MlxfnIoError::UnsupportedDevice {
                        device_type,
                        stream_index,
                    });
                }
                let primitive_name = self.read_string()?;
                let primitive = self.read_primitive(primitive_name)?;
                let siblings = self.read_u64()?;
                if siblings != 0 {
                    return Err(MlxfnIoError::UnsupportedMultiOutputPrimitive {
                        primitive: primitive.name().to_string(),
                        siblings,
                    });
                }
                let spec = self.read_tensor_spec()?;
                tape.push(MlxfnTapeEntry::Primitive {
                    id,
                    stream_index,
                    input_ids,
                    primitive,
                    spec,
                });
            } else {
                let is_constant = self.read_bool()?;
                if is_constant {
                    let spec = self.read_tensor_spec()?;
                    if spec.dtype != DType::F32 {
                        return Err(MlxfnIoError::UnsupportedConstantDType { dtype: spec.dtype });
                    }
                    let values = self.read_f32_values(spec.shape.element_count())?;
                    tape.push(MlxfnTapeEntry::Constant { id, spec, values });
                } else {
                    tape.push(MlxfnTapeEntry::Input { id });
                }
            }
        }
        Ok(MlxfnTrace {
            kwarg_keys,
            trace_input_ids,
            trace_inputs,
            trace_output_ids,
            tape,
        })
    }

    fn read_stream(&mut self) -> Result<(i32, i32), MlxfnIoError> {
        let stream_index = self.read_i32()?;
        let device_type = self.read_i32()?;
        let _device_index = self.read_i32()?;
        Ok((stream_index, device_type))
    }

    fn read_primitive(&mut self, primitive_name: String) -> Result<MlxfnPrimitive, MlxfnIoError> {
        match primitive_name.as_str() {
            "Add" => Ok(MlxfnPrimitive::Add),
            "Multiply" => Ok(MlxfnPrimitive::Multiply),
            "Matmul" => Ok(MlxfnPrimitive::Matmul),
            "StopGradient" => Ok(MlxfnPrimitive::StopGradient),
            "AsType" => Ok(MlxfnPrimitive::AsType(self.read_dtype()?)),
            "Reshape" => Ok(MlxfnPrimitive::Reshape(self.read_shape()?)),
            "Transpose" => Ok(MlxfnPrimitive::Transpose(self.read_usize_vec_from_i32()?)),
            "Concatenate" => Ok(MlxfnPrimitive::Concatenate(
                usize::try_from(self.read_i32()?).map_err(|_| MlxfnIoError::InvalidArtifact {
                    detail: String::from("concatenate axis was negative"),
                })?,
            )),
            "Broadcast" => Ok(MlxfnPrimitive::Broadcast(self.read_shape()?)),
            "Sum" => {
                let reduce_type = self.read_i32()?;
                if reduce_type != MLX_REDUCE_SUM {
                    return Err(MlxfnIoError::UnsupportedPrimitive {
                        primitive: format!("Sum state reduce_type={reduce_type}"),
                    });
                }
                Ok(MlxfnPrimitive::ReduceSum(self.read_usize_vec_from_i32()?))
            }
            "Slice" => Ok(MlxfnPrimitive::Slice {
                start: self.read_usize_vec_from_i32()?,
                end: self.read_usize_vec_from_i32()?,
                strides: self.read_usize_vec_from_i32()?,
            }),
            _ => Err(MlxfnIoError::UnsupportedPrimitive {
                primitive: primitive_name,
            }),
        }
    }

    fn read_tensor_spec(&mut self) -> Result<MlxfnTensorSpec, MlxfnIoError> {
        Ok(MlxfnTensorSpec {
            shape: self.read_shape()?,
            dtype: self.read_dtype()?,
        })
    }

    fn read_dtype(&mut self) -> Result<DType, MlxfnIoError> {
        let value = self.read_i32()?;
        let size = self.read_u8()?;
        psionic_dtype_from_mlx(value, size)
    }

    fn read_shape(&mut self) -> Result<Shape, MlxfnIoError> {
        let len = self.read_u64()?;
        let mut dims = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let dim = self.read_i32()?;
            dims.push(
                usize::try_from(dim).map_err(|_| MlxfnIoError::InvalidArtifact {
                    detail: String::from("shape dimension was negative"),
                })?,
            );
        }
        Ok(Shape::new(dims))
    }

    fn read_usize_vec_from_i32(&mut self) -> Result<Vec<usize>, MlxfnIoError> {
        let len = self.read_u64()?;
        let mut values = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let value = self.read_i32()?;
            values.push(
                usize::try_from(value).map_err(|_| MlxfnIoError::InvalidArtifact {
                    detail: String::from("vector element was negative"),
                })?,
            );
        }
        Ok(values)
    }

    fn read_f32_values(&mut self, count: usize) -> Result<Vec<f32>, MlxfnIoError> {
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.read_f32()?);
        }
        Ok(values)
    }

    fn read_vec<T, F>(&mut self, mut read_one: F) -> Result<Vec<T>, MlxfnIoError>
    where
        F: FnMut(&mut Self) -> Result<T, MlxfnIoError>,
    {
        let len = self.read_u64()?;
        let mut values = Vec::with_capacity(len as usize);
        for _ in 0..len {
            values.push(read_one(self)?);
        }
        Ok(values)
    }

    fn read_string(&mut self) -> Result<String, MlxfnIoError> {
        let len = self.read_u64()?;
        let slice = self.take(len as usize)?;
        String::from_utf8(slice.to_vec()).map_err(|error| MlxfnIoError::InvalidArtifact {
            detail: format!("invalid UTF-8 string payload: {error}"),
        })
    }

    fn read_bool(&mut self) -> Result<bool, MlxfnIoError> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(MlxfnIoError::InvalidArtifact {
                detail: format!("invalid bool tag `{value}`"),
            }),
        }
    }

    fn read_u8(&mut self) -> Result<u8, MlxfnIoError> {
        let slice = self.take(1)?;
        Ok(slice[0])
    }

    fn read_i32(&mut self) -> Result<i32, MlxfnIoError> {
        let slice = self.take(4)?;
        let mut bytes = [0_u8; 4];
        bytes.copy_from_slice(slice);
        Ok(i32::from_le_bytes(bytes))
    }

    fn read_u64(&mut self) -> Result<u64, MlxfnIoError> {
        let slice = self.take(8)?;
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(slice);
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_f32(&mut self) -> Result<f32, MlxfnIoError> {
        let slice = self.take(4)?;
        let mut bytes = [0_u8; 4];
        bytes.copy_from_slice(slice);
        Ok(f32::from_le_bytes(bytes))
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], MlxfnIoError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| MlxfnIoError::InvalidArtifact {
                detail: String::from("offset overflow while reading artifact"),
            })?;
        if end > self.bytes.len() {
            return Err(MlxfnIoError::InvalidArtifact {
                detail: String::from("unexpected end of file while reading artifact"),
            });
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }
}

fn tensor_ids_to_u64(ids: &[TensorId]) -> Vec<u64> {
    ids.iter().map(|id| u64::from(id.0)).collect()
}

fn input_shape(
    artifact: &FunctionArtifact,
    input_id: Option<TensorId>,
) -> Result<Shape, MlxfnIoError> {
    let input_id = input_id.ok_or_else(|| MlxfnIoError::InvalidArtifact {
        detail: String::from("expected one input tensor but none was present"),
    })?;
    artifact
        .graph
        .node(input_id)
        .map(|node| node.tensor().spec().shape().clone())
        .ok_or_else(|| MlxfnIoError::InvalidArtifact {
            detail: format!("unknown tensor `{input_id}` while deriving MLX compatibility state"),
        })
}

fn ensure_tensor_matches_spec(tensor: &Tensor, spec: &MlxfnTensorSpec) -> Result<(), MlxfnIoError> {
    if tensor.spec().shape() != &spec.shape || tensor.spec().dtype() != spec.dtype {
        return Err(MlxfnIoError::InvalidArtifact {
            detail: format!(
                "primitive materialized tensor {} with spec {} {:?}, expected {} {:?}",
                tensor.id(),
                tensor.spec().shape(),
                tensor.spec().dtype(),
                spec.shape,
                spec.dtype
            ),
        });
    }
    Ok(())
}

fn mlxfn_slice_vectors(
    input_shape: &Shape,
    axis: usize,
    start: usize,
    end: usize,
) -> Result<SliceVectors, MlxfnIoError> {
    if axis >= input_shape.rank() {
        return Err(MlxfnIoError::InvalidArtifact {
            detail: format!(
                "slice axis {axis} was outside input rank {}",
                input_shape.rank()
            ),
        });
    }
    let mut start_indices = vec![0; input_shape.rank()];
    let mut end_indices = input_shape.dims().to_vec();
    let strides = vec![1; input_shape.rank()];
    start_indices[axis] = start;
    end_indices[axis] = end;
    Ok((start_indices, end_indices, strides))
}

fn decode_single_axis_slice(
    input_shape: &Shape,
    start: &[usize],
    end: &[usize],
    strides: &[usize],
) -> Result<(usize, usize, usize), MlxfnIoError> {
    if start.len() != input_shape.rank()
        || end.len() != input_shape.rank()
        || strides.len() != input_shape.rank()
    {
        return Err(MlxfnIoError::InvalidArtifact {
            detail: String::from("slice state rank mismatch"),
        });
    }
    let mut changed_axis = None;
    for axis in 0..input_shape.rank() {
        if strides[axis] != 1 {
            return Err(MlxfnIoError::UnsupportedPrimitive {
                primitive: String::from("Slice with stride != 1"),
            });
        }
        let full_start = 0;
        let full_end = input_shape.dims()[axis];
        if start[axis] != full_start || end[axis] != full_end {
            if changed_axis.is_some() {
                return Err(MlxfnIoError::UnsupportedPrimitive {
                    primitive: String::from("Slice touching more than one axis"),
                });
            }
            changed_axis = Some((axis, start[axis], end[axis]));
        }
    }
    changed_axis.ok_or_else(|| MlxfnIoError::UnsupportedPrimitive {
        primitive: String::from("Slice with no effective sliced axis"),
    })
}

fn to_i32(value: usize, label: &str) -> Result<i32, MlxfnIoError> {
    i32::try_from(value).map_err(|_| MlxfnIoError::InvalidArtifact {
        detail: format!("{label} `{value}` exceeded i32 range"),
    })
}

fn mlx_dtype_from_psionic(dtype: DType) -> Result<(i32, u8), MlxfnIoError> {
    match dtype {
        DType::F32 => Ok((10, 4)),
        DType::F16 => Ok((9, 2)),
        DType::BF16 => Ok((12, 2)),
        DType::I8 => Ok((5, 1)),
    }
}

fn psionic_dtype_from_mlx(dtype_value: i32, dtype_size: u8) -> Result<DType, MlxfnIoError> {
    match (dtype_value, dtype_size) {
        (10, 4) => Ok(DType::F32),
        (9, 2) => Ok(DType::F16),
        (12, 2) => Ok(DType::BF16),
        (5, 1) => Ok(DType::I8),
        _ => Err(MlxfnIoError::UnsupportedImportDType {
            dtype_value,
            dtype_size,
            detail: String::from(
                "the bounded Psionic `.mlxfn` bridge supports only f32, f16, bf16, and i8",
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MlxfnIoError, decode_mlxfn_function_artifact, encode_mlxfn_function_artifact,
        load_mlxfn_function_artifact_path, save_mlxfn_function_artifact_path,
    };
    use crate::{
        FunctionArtifact, FunctionArtifactDirection, FunctionCompileBundle, FunctionIoError,
    };
    use psionic_compiler::{
        CompileShapeMode, DeploymentArtifactFormat, compile_graph_artifacts,
        compile_trace_family_identity,
    };
    use psionic_core::{DType, Device, PsionicRefusalCode, Shape};
    use psionic_ir::{Graph, GraphBuilder};
    use tempfile::NamedTempFile;

    fn ensure(condition: bool, message: impl Into<String>) -> Result<(), MlxfnIoError> {
        if condition {
            Ok(())
        } else {
            Err(MlxfnIoError::InvalidArtifact {
                detail: message.into(),
            })
        }
    }

    fn seeded_supported_graph() -> Result<Graph, FunctionIoError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input_0", Shape::new(vec![2, 2]), DType::F32);
        let bias = builder
            .constant_f32(Shape::new(vec![2]), vec![0.5, 1.5])
            .map_err(|error| FunctionIoError::Serialization {
                operation: "build seeded supported graph",
                message: error.to_string(),
            })?;
        let expanded = builder
            .expand(&bias, Shape::new(vec![2, 2]))
            .map_err(|error| FunctionIoError::Serialization {
                operation: "build seeded supported graph",
                message: error.to_string(),
            })?;
        let shifted =
            builder
                .add(&input, &expanded)
                .map_err(|error| FunctionIoError::Serialization {
                    operation: "build seeded supported graph",
                    message: error.to_string(),
                })?;
        let reduced = builder.reduce_sum_axis(&shifted, 1).map_err(|error| {
            FunctionIoError::Serialization {
                operation: "build seeded supported graph",
                message: error.to_string(),
            }
        })?;
        Ok(builder.finish(vec![reduced]))
    }

    fn seeded_unsupported_select_graph() -> Result<Graph, FunctionIoError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input_0", Shape::new(vec![2, 2]), DType::F32);
        let selected =
            builder
                .select(&input, 0, 0)
                .map_err(|error| FunctionIoError::Serialization {
                    operation: "build seeded unsupported graph",
                    message: error.to_string(),
                })?;
        Ok(builder.finish(vec![selected]))
    }

    #[test]
    fn mlxfn_roundtrips_supported_subset_and_strips_compile_bundle() -> Result<(), MlxfnIoError> {
        let graph = seeded_supported_graph()?;
        let export_contract = graph
            .exportable_graph_contract("main")
            .map_err(FunctionIoError::from)
            .map_err(MlxfnIoError::from)?;
        let artifacts =
            compile_graph_artifacts(&graph).map_err(|error| FunctionIoError::Serialization {
                operation: "compile seeded supported graph",
                message: error.to_string(),
            })?;
        let trace_family_identity =
            compile_trace_family_identity(&artifacts, CompileShapeMode::ConcreteOnly)
                .map_err(FunctionIoError::from)
                .map_err(MlxfnIoError::from)?;
        let deployment_artifact_contract = artifacts
            .deployment_artifact_contract(
                &export_contract,
                "supported_bundle",
                DeploymentArtifactFormat::ExecutionPlanBundle,
            )
            .map_err(FunctionIoError::from)
            .map_err(MlxfnIoError::from)?;
        let compile_bundle = FunctionCompileBundle::new(
            artifacts,
            Some(trace_family_identity),
            Some(deployment_artifact_contract),
        )?;
        let artifact =
            FunctionArtifact::from_graph_with_compile_bundle(&graph, "main", compile_bundle)?;
        let (bytes, export_receipt) = encode_mlxfn_function_artifact(&artifact)?;
        let (decoded, import_receipt) = decode_mlxfn_function_artifact(&bytes, "main")?;
        let file = NamedTempFile::new().map_err(|error| MlxfnIoError::Io {
            path: String::from("<tempfile>"),
            operation: "create",
            message: error.to_string(),
        })?;
        let saved_receipt = save_mlxfn_function_artifact_path(&artifact, file.path())?;
        let (loaded, loaded_receipt) = load_mlxfn_function_artifact_path(file.path(), "main")?;

        ensure(
            export_receipt.direction == FunctionArtifactDirection::Export,
            "unexpected export direction",
        )?;
        ensure(
            import_receipt.direction == FunctionArtifactDirection::Import,
            "unexpected import direction",
        )?;
        ensure(
            export_receipt.stripped_compile_bundle,
            "expected compile bundle stripping on `.mlxfn` export",
        )?;
        ensure(
            decoded.compile_bundle.is_none(),
            "imported `.mlxfn` unexpectedly retained a compile bundle",
        )?;
        ensure(
            decoded.graph.stable_digest() == graph.stable_digest(),
            "decoded graph digest drifted",
        )?;
        ensure(
            loaded.graph.stable_digest() == graph.stable_digest(),
            "loaded graph digest drifted",
        )?;
        ensure(
            saved_receipt.artifact_sha256 == loaded_receipt.artifact_sha256,
            "path save/load receipt digest drifted",
        )?;

        Ok(())
    }

    #[test]
    fn mlxfn_refuses_shapeless_and_multiple_trace_headers() -> Result<(), MlxfnIoError> {
        let graph = seeded_supported_graph()?;
        let artifact = FunctionArtifact::from_graph(&graph, "main")?;
        let (mut bytes, _) = encode_mlxfn_function_artifact(&artifact)?;

        let version_len_offset = 0;
        let version_len =
            u64::from_le_bytes(bytes[version_len_offset..8].try_into().map_err(|_| {
                MlxfnIoError::InvalidArtifact {
                    detail: String::from("failed to parse version length during test"),
                }
            })?);
        let count_offset =
            8 + usize::try_from(version_len).map_err(|_| MlxfnIoError::InvalidArtifact {
                detail: String::from("failed to convert version length during test"),
            })?;
        bytes[count_offset..count_offset + 4].copy_from_slice(&2_i32.to_le_bytes());
        let error = match decode_mlxfn_function_artifact(&bytes, "main") {
            Ok(_) => {
                return Err(MlxfnIoError::InvalidArtifact {
                    detail: String::from(
                        "multiple-trace header unexpectedly decoded as supported `.mlxfn`",
                    ),
                });
            }
            Err(error) => error,
        };
        ensure(
            matches!(error, MlxfnIoError::UnsupportedFunctionCount { .. }),
            "multiple traces did not return UnsupportedFunctionCount",
        )?;

        let (mut bytes, _) = encode_mlxfn_function_artifact(&artifact)?;
        bytes[count_offset + 4] = 1;
        let error = match decode_mlxfn_function_artifact(&bytes, "main") {
            Ok(_) => {
                return Err(MlxfnIoError::InvalidArtifact {
                    detail: String::from(
                        "shapeless header unexpectedly decoded as supported `.mlxfn`",
                    ),
                });
            }
            Err(error) => error,
        };
        ensure(
            matches!(error, MlxfnIoError::UnsupportedShapeless),
            "shapeless trace did not return UnsupportedShapeless",
        )?;
        Ok(())
    }

    #[test]
    fn mlxfn_refuses_unsupported_primitives_and_maps_refusals() -> Result<(), MlxfnIoError> {
        let graph = seeded_unsupported_select_graph()?;
        let artifact = FunctionArtifact::from_graph(&graph, "main")?;
        let error = match encode_mlxfn_function_artifact(&artifact) {
            Ok(_) => {
                return Err(MlxfnIoError::InvalidArtifact {
                    detail: String::from(
                        "select graph unexpectedly exported through bounded `.mlxfn` support",
                    ),
                });
            }
            Err(error) => error,
        };
        let refusal = error
            .refusal()
            .ok_or_else(|| MlxfnIoError::InvalidArtifact {
                detail: String::from("unsupported primitive did not map to a refusal"),
            })?;
        ensure(
            matches!(error, MlxfnIoError::UnsupportedPrimitive { .. }),
            "unsupported primitive did not return UnsupportedPrimitive",
        )?;
        ensure(
            refusal.code == PsionicRefusalCode::UnsupportedOp,
            "unsupported primitive refusal code drifted",
        )?;
        Ok(())
    }

    #[test]
    fn mlxfn_import_refuses_unknown_primitives() -> Result<(), MlxfnIoError> {
        let graph = seeded_supported_graph()?;
        let artifact = FunctionArtifact::from_graph(&graph, "main")?;
        let (mut bytes, _) = encode_mlxfn_function_artifact(&artifact)?;
        if let Some(position) = bytes.windows(3).position(|window| window == b"Add") {
            bytes[position..position + 3].copy_from_slice(b"Abs");
        } else {
            return Err(MlxfnIoError::InvalidArtifact {
                detail: String::from(
                    "failed to locate `Add` primitive while mutating test artifact",
                ),
            });
        }
        let error = match decode_mlxfn_function_artifact(&bytes, "main") {
            Ok(_) => {
                return Err(MlxfnIoError::InvalidArtifact {
                    detail: String::from(
                        "mutated unsupported primitive unexpectedly decoded successfully",
                    ),
                });
            }
            Err(error) => error,
        };
        ensure(
            matches!(error, MlxfnIoError::UnsupportedPrimitive { .. }),
            "mutated primitive did not return UnsupportedPrimitive",
        )?;
        Ok(())
    }
}
