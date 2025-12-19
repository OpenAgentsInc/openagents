use maud::{Markup, html};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlanPhase {
    Explore,
    Design,
    Review,
    Final,
    Exit,
}

impl PlanPhase {
    fn label(&self) -> &'static str {
        match self {
            PlanPhase::Explore => "explore",
            PlanPhase::Design => "design",
            PlanPhase::Review => "review",
            PlanPhase::Final => "final",
            PlanPhase::Exit => "exit",
        }
    }

    fn index(&self) -> usize {
        match self {
            PlanPhase::Explore => 0,
            PlanPhase::Design => 1,
            PlanPhase::Review => 2,
            PlanPhase::Final => 3,
            PlanPhase::Exit => 4,
        }
    }
}

const ALL_PHASES: [PlanPhase; 5] = [
    PlanPhase::Explore,
    PlanPhase::Design,
    PlanPhase::Review,
    PlanPhase::Final,
    PlanPhase::Exit,
];

pub fn phase_indicator(current_phase: PlanPhase) -> Markup {
    let current_idx = current_phase.index();

    html! {
        div class="inline-flex flex-col gap-3 border border-border bg-card px-4 py-3" {
            div class="text-xs text-muted-foreground tracking-widest uppercase" { "PLAN MODE" }
            div class="flex items-center gap-4" {
                @for phase in ALL_PHASES.iter() {
                    @let is_complete = phase.index() < current_idx;
                    @let is_current = phase.index() == current_idx;
                    @let color = if is_complete || is_current {
                        "text-blue"
                    } else {
                        "text-muted-foreground opacity-60"
                    };
                    @let dot = if is_complete || is_current { "\u{25CF}" } else { "\u{25CB}" };

                    span class={ "flex items-center gap-1 cursor-pointer " (if is_current { "font-semibold" } else { "" }) } {
                        span class={ "text-xs " (color) } { (dot) }
                        span class={ "text-xs " (color) } { (phase.label()) }
                    }
                }
            }
        }
    }
}
