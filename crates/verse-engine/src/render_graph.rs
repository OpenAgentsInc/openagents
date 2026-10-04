//! Portable admission and deterministic scheduling for render passes.
use std::collections::BTreeSet;

/// A resource available before execution or produced by an admitted pass.
#[derive(Clone, Debug)]
pub struct Resource {
    pub imported: bool,
}

/// Resource indices and explicit predecessor pass indices.
#[derive(Clone, Debug, Default)]
pub struct Pass {
    pub reads: Vec<usize>,
    pub writes: Vec<usize>,
    pub after: Vec<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub resources: Vec<Resource>,
    pub passes: Vec<Pass>,
}

/// An immutable schedule admitted against its complete source graph.
#[derive(Clone, Debug)]
pub struct Schedule {
    graph: Graph,
    order: Vec<usize>,
}
impl Schedule {
    pub fn graph(&self) -> &Graph {
        &self.graph
    }
    pub fn order(&self) -> &[usize] {
        &self.order
    }
}

impl Graph {
    /// Rejects uninitialized reads, ambiguous resource hazards, and cycles.
    pub fn admit(self) -> Result<Schedule, String> {
        if self.resources.len() > 256 || self.passes.len() > 256 {
            return Err("Render graph exceeds 256 resources or passes".into());
        }
        let count = self.passes.len();
        let mut dependencies = vec![BTreeSet::new(); count];
        for (i, pass) in self.passes.iter().enumerate() {
            if pass.reads.len() > 256 || pass.writes.len() > 256 || pass.after.len() > 256 {
                return Err("Render pass exceeds the access or dependency capacity".into());
            }
            for slots in [&pass.reads, &pass.writes] {
                let mut seen = BTreeSet::new();
                for &slot in slots {
                    if slot >= self.resources.len() || !seen.insert(slot) {
                        return Err("Render pass has an invalid or repeated resource access".into());
                    }
                }
            }
            for &predecessor in &pass.after {
                if predecessor >= count || predecessor == i || !dependencies[i].insert(predecessor)
                {
                    return Err("Render pass has an invalid or repeated dependency".into());
                }
            }
        }
        let mut order = Vec::with_capacity(count);
        let mut completed = BTreeSet::new();
        while order.len() < count {
            let Some(next) = (0..count)
                .find(|i| !completed.contains(i) && dependencies[*i].is_subset(&completed))
            else {
                return Err("Render graph contains a dependency cycle".into());
            };
            completed.insert(next);
            order.push(next);
        }
        let mut ancestors = vec![BTreeSet::new(); count];
        for &i in &order {
            for &parent in &dependencies[i] {
                ancestors[i].insert(parent);
                let inherited = ancestors[parent].clone();
                ancestors[i].extend(inherited);
            }
        }
        for i in 0..count {
            for j in i + 1..count {
                let a = &self.passes[i];
                let b = &self.passes[j];
                let conflict = a
                    .writes
                    .iter()
                    .any(|r| b.reads.contains(r) || b.writes.contains(r))
                    || b.writes.iter().any(|r| a.reads.contains(r));
                if conflict && !ancestors[i].contains(&j) && !ancestors[j].contains(&i) {
                    return Err("Render resource hazard lacks an explicit dependency".into());
                }
            }
        }
        let mut initialized: BTreeSet<_> = self
            .resources
            .iter()
            .enumerate()
            .filter_map(|(i, resource)| resource.imported.then_some(i))
            .collect();
        for &i in &order {
            let pass = &self.passes[i];
            if pass.reads.iter().any(|r| !initialized.contains(r)) {
                return Err("Render pass reads a resource without an available producer".into());
            }
            initialized.extend(pass.writes.iter().copied());
        }
        Ok(Schedule { graph: self, order })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chain() -> Graph {
        Graph {
            resources: vec![Resource { imported: true }, Resource { imported: false }],
            passes: vec![
                Pass {
                    reads: vec![0],
                    writes: vec![1],
                    after: vec![2],
                },
                Pass {
                    reads: vec![1],
                    after: vec![0],
                    ..Default::default()
                },
                Pass::default(),
            ],
        }
    }
    #[test]
    fn schedules_producers_before_consumers_with_stable_ties() {
        let admitted = chain().admit().unwrap();
        assert_eq!(admitted.order(), &[2, 0, 1]);
        assert_eq!(admitted.graph().passes.len(), 3);
    }
    #[test]
    fn refuses_uninitialized_reads_and_unordered_hazards() {
        let mut graph = chain();
        graph.resources[0].imported = false;
        assert!(graph.admit().unwrap_err().contains("producer"));
        let mut graph = chain();
        graph.passes[1].after.clear();
        assert!(graph.admit().unwrap_err().contains("hazard"));
    }
    #[test]
    fn refuses_cycles_invalid_indices_and_capacity_overflow() {
        let mut graph = chain();
        graph.passes[2].after.push(1);
        assert!(graph.admit().unwrap_err().contains("cycle"));
        let mut graph = chain();
        graph.passes[0].writes.push(5);
        assert!(graph.admit().is_err());
        let graph = Graph {
            passes: vec![Pass::default(); 257],
            ..Default::default()
        };
        assert!(graph.admit().is_err());
    }
    #[test]
    fn permits_read_sharing_and_orders_read_modify_write() {
        let mut graph = chain();
        graph.passes[1].writes.push(1);
        graph.passes.push(Pass {
            reads: vec![0],
            ..Default::default()
        });
        assert!(graph.admit().is_ok());
    }
}

/// Work performed by the chamber backend for an admitted pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChamberPass {
    RefreshShadow {
        layer: usize,
    },
    CopyShadow {
        layer: usize,
    },
    DrawShadow {
        layer: usize,
    },
    /// Draw multisample color/depth and resolve color in the same GPU pass.
    WorldResolve,
    Overlay,
    Readback,
}

/// An admitted plan with immutable pass actions and resource dependencies.
#[derive(Clone, Debug)]
pub struct ChamberPlan {
    schedule: Schedule,
    actions: Vec<ChamberPass>,
}
impl ChamberPlan {
    /// Each boolean declares whether the corresponding cached cube face needs refresh.
    pub fn build(refresh: &[bool], capture: bool) -> Result<Self, String> {
        if refresh.len() > 24 || !refresh.len().is_multiple_of(6) {
            return Err("Chamber render plan requires at most four complete shadow cubes".into());
        }
        let mut graph = Graph::default();
        let mut actions = Vec::new();
        let mut shadow_resources = Vec::new();
        let mut shadow_producers = Vec::new();
        for (layer, &refresh) in refresh.iter().enumerate() {
            let cached = graph.resources.len();
            graph.resources.push(Resource { imported: !refresh });
            let dynamic = graph.resources.len();
            graph.resources.push(Resource { imported: false });
            let mut after = Vec::new();
            if refresh {
                after.push(graph.passes.len());
                graph.passes.push(Pass {
                    writes: vec![cached],
                    ..Default::default()
                });
                actions.push(ChamberPass::RefreshShadow { layer });
            }
            let copied = graph.passes.len();
            graph.passes.push(Pass {
                reads: vec![cached],
                writes: vec![dynamic],
                after,
            });
            actions.push(ChamberPass::CopyShadow { layer });
            shadow_producers.push(graph.passes.len());
            graph.passes.push(Pass {
                reads: vec![dynamic],
                writes: vec![dynamic],
                after: vec![copied],
            });
            actions.push(ChamberPass::DrawShadow { layer });
            shadow_resources.push(dynamic);
        }
        let color = graph.resources.len();
        graph.resources.push(Resource { imported: false });
        let multisample = graph.resources.len();
        graph.resources.push(Resource { imported: false });
        let depth = graph.resources.len();
        graph.resources.push(Resource { imported: false });
        let world = graph.passes.len();
        graph.passes.push(Pass {
            reads: shadow_resources,
            writes: vec![color, multisample, depth],
            after: shadow_producers,
        });
        actions.push(ChamberPass::WorldResolve);
        let overlay = graph.passes.len();
        graph.passes.push(Pass {
            reads: vec![color],
            writes: vec![color],
            after: vec![world],
        });
        actions.push(ChamberPass::Overlay);
        if capture {
            let readback = graph.resources.len();
            graph.resources.push(Resource { imported: false });
            graph.passes.push(Pass {
                reads: vec![color],
                writes: vec![readback],
                after: vec![overlay],
            });
            actions.push(ChamberPass::Readback);
        }
        Ok(Self {
            schedule: graph.admit()?,
            actions,
        })
    }
    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }
    pub fn actions(&self) -> impl Iterator<Item = ChamberPass> + '_ {
        self.schedule.order.iter().map(|&i| self.actions[i])
    }
}

#[cfg(test)]
mod chamber_tests {
    use super::*;
    #[test]
    fn cache_refresh_precedes_copy_and_dynamic_draw() {
        let plan = ChamberPlan::build(&[true; 6], true).unwrap();
        let actions: Vec<_> = plan.actions().collect();
        assert_eq!(
            &actions[..3],
            &[
                ChamberPass::RefreshShadow { layer: 0 },
                ChamberPass::CopyShadow { layer: 0 },
                ChamberPass::DrawShadow { layer: 0 }
            ]
        );
        assert_eq!(
            &actions[actions.len() - 3..],
            &[
                ChamberPass::WorldResolve,
                ChamberPass::Overlay,
                ChamberPass::Readback
            ]
        );
    }
    #[test]
    fn cached_faces_skip_refresh_but_keep_dynamic_shadows() {
        let plan = ChamberPlan::build(&[false; 24], false).unwrap();
        assert_eq!(plan.actions().count(), 50);
        assert!(
            !plan
                .actions()
                .any(|a| matches!(a, ChamberPass::RefreshShadow { .. } | ChamberPass::Readback))
        );
    }
    #[test]
    fn every_supported_cache_mask_is_admitted() {
        for mask in 0..64 {
            let refresh: Vec<_> = (0..6).map(|bit| mask & (1 << bit) != 0).collect();
            assert!(ChamberPlan::build(&refresh, mask % 2 == 0).is_ok());
        }
        assert!(ChamberPlan::build(&[], true).is_ok());
        assert!(ChamberPlan::build(&[true; 1], false).is_err());
        assert!(ChamberPlan::build(&[true; 30], false).is_err());
    }
}
