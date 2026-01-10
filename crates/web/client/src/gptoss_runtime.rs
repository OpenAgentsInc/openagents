#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use bytemuck::{cast_slice, Pod, Zeroable};
use futures::channel::oneshot;
use gloo_timers::future::TimeoutFuture;
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wgpu::util::DeviceExt;
use wasm_bindgen_futures::spawn_local;
use js_sys;
use web_sys;

use crate::gguf_web::{
    fetch_and_parse_index_source, fetch_range_source, fetch_range_with_total_source,
    pick_gguf_file, GgufIndex, GgufSource, GgufTensor,
};
use crate::gptoss_tokenizer::GptOssTokenizer;
use crate::gptoss_viz::{
    clear_gptoss_events, push_gptoss_event, GptOssInferenceTelemetry, GptOssTelemetry,
    GptOssTokenCandidate, StageStatus,
};
use crate::state::{AppState, GpuContext};

include!("gptoss_runtime/config.rs");
include!("gptoss_runtime/cache.rs");
include!("gptoss_runtime/runtime.rs");
include!("gptoss_runtime/loader.rs");
include!("gptoss_runtime/meta.rs");
include!("gptoss_runtime/inputs.rs");
include!("gptoss_runtime/sampling.rs");
include!("gptoss_runtime/probes.rs");
include!("gptoss_runtime/ops.rs");
include!("gptoss_runtime/shaders.rs");
