pub mod copro;
pub mod gepa;
pub mod mipro;
pub mod pareto;

pub use copro::*;
pub use gepa::*;
pub use mipro::*;
pub use pareto::*;

use crate::{
    core::{Module, Optimizable},
    data::example::Example,
    evaluate::Evaluator,
};
use anyhow::Result;

#[allow(async_fn_in_trait)]
pub trait Optimizer {
    async fn compile<M>(&self, module: &mut M, trainset: Vec<Example>) -> Result<()>
    where
        M: Module + Optimizable + Evaluator;
}
