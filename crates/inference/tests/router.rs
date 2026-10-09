//! The router's planning rules (`docs/inference/gateway.md`, section 5).

use std::collections::BTreeMap;

use inference::ApiError;
use inference::CreateResponse;
use inference::error::ErrorType;
use inference::meter::{Basis, CreditAccount, ErrorClass, Ledger, Rate, RateCard, RateRow};
use inference::openagents::{MaxPrice, Payer, Privacy, RequestOptions, RoutePreferences, Sort};
use inference::request::{FunctionTool, Input, Tool};
use inference::router::{
    BENCH_MS, Bench, Capabilities, ClassJudge, ClassTable, Context, Dropped, Offering, Plan,
    PriceLimit, Scores, TaskClass, fall_back, micros_usd, plan, usd_micros,
};

const FLASH: &str = "google/gemini-3.8-flash";
const LITE: &str = "google/gemini-3.8-flash-lite";
const PRO: &str = "google/gemini-3.8-pro";
const GLM: &str = "zai/glm-5.3-flash";
const NOW: u64 = 1_800_000_000_000;
const DAY: u64 = 86_400_000;

fn offering(
    upstream: &str,
    model: &str,
    tools: bool,
    zdr: bool,
    account: Option<&str>,
) -> Offering {
    Offering {
        upstream: upstream.into(),
        model: model.into(),
        capabilities: Capabilities {
            tools,
            json_schema: true,
            images: model.starts_with("google/"),
            files: false,
            reasoning: true,
            context: if model.starts_with("google/") || model == GLM {
                1_000_000
            } else {
                400_000
            },
            max_output: 0,
        },
        zero_retention: zdr,
        payer: Payer::Ours,
        account: account.map(str::to_owned),
    }
}

fn offerings() -> Vec<Offering> {
    vec![
        offering("vertex", FLASH, true, true, Some("google")),
        offering("vertex", LITE, true, true, Some("google")),
        offering("vertex", PRO, true, true, Some("google")),
        offering("zai", GLM, true, true, Some("zai")),
        offering("pro", "openai/gpt-5.6-luna", false, true, Some("pro")),
        offering("pro", "openai/gpt-5.6-terra", false, true, Some("pro")),
        offering("pro", "openai/gpt-5.6-sol", false, true, Some("pro")),
        offering("openrouter", FLASH, true, true, None),
        offering("openrouter", PRO, true, true, None),
        offering("vercel", FLASH, true, false, None),
    ]
}

fn row(upstream: &str, model: &str, input: u64, output: u64) -> RateRow {
    RateRow {
        upstream: upstream.into(),
        model: model.into(),
        currency: "USD".into(),
        input,
        cached_input: None,
        cache_write: None,
        output,
        margin_bps: 500,
    }
}

fn card() -> RateCard {
    RateCard::new([
        row("vertex", FLASH, 300_000, 2_500_000),
        row("vertex", LITE, 100_000, 400_000),
        row("vertex", PRO, 1_250_000, 10_000_000),
        row("zai", GLM, 200_000, 1_100_000),
        row("pro", "openai/gpt-5.6-luna", 50_000, 400_000),
        row("pro", "openai/gpt-5.6-terra", 250_000, 2_000_000),
        row("pro", "openai/gpt-5.6-sol", 1_250_000, 10_000_000),
        row("openrouter", FLASH, 300_000, 2_500_000),
        row("openrouter", PRO, 1_250_000, 10_000_000),
        row("vercel", FLASH, 290_000, 2_400_000),
    ])
}

fn account(id: &str, basis: Basis, balance: u64, expires_in_days: Option<u64>) -> CreditAccount {
    CreditAccount {
        id: id.into(),
        upstream: id.into(),
        currency: "USD".into(),
        granted: 1_000_000_000,
        balance,
        expires_at_ms: expires_in_days.map(|days| NOW + days * DAY),
        basis,
    }
}

fn ledger() -> Ledger {
    Ledger::new([
        account("google", Basis::Prepaid, 500_000_000, Some(200)),
        account("zai", Basis::Prepaid, 100_000_000, Some(60)),
        account("pro", Basis::FreeCapacity, 0, None),
    ])
}

struct World {
    offerings: Vec<Offering>,
    classes: ClassTable,
    card: RateCard,
    ledger: Ledger,
    rates: Vec<Rate>,
    scores: Scores,
    bench: Bench,
    limits: PriceLimit,
}

impl World {
    fn new() -> Self {
        Self {
            offerings: offerings(),
            classes: ClassTable::default(),
            card: card(),
            ledger: ledger(),
            rates: Vec::new(),
            scores: Scores::default(),
            bench: Bench::default(),
            limits: PriceLimit::default(),
        }
    }

    fn plan_with(
        &self,
        request: &CreateResponse,
        judge: Option<&dyn ClassJudge>,
    ) -> Result<Plan, ApiError> {
        plan(
            request,
            &Context {
                offerings: &self.offerings,
                classes: &self.classes,
                card: &self.card,
                ledger: &self.ledger,
                rates: &self.rates,
                scores: &self.scores,
                bench: &self.bench,
                limits: self.limits,
                judge,
                now_ms: NOW,
            },
        )
    }

    fn plan(&self, request: &CreateResponse) -> Result<Plan, ApiError> {
        self.plan_with(request, None)
    }

    /// The planned (upstream, model) pairs.
    fn route(&self, request: &CreateResponse) -> Vec<(String, String)> {
        self.plan(request)
            .unwrap()
            .attempts
            .into_iter()
            .map(|candidate| (candidate.upstream, candidate.model))
            .collect()
    }
}

fn ask(model: &str) -> CreateResponse {
    CreateResponse {
        model: Some(model.into()),
        input: Some(Input::Text("Say hi.".into())),
        ..CreateResponse::default()
    }
}

fn with(model: &str, options: RequestOptions) -> CreateResponse {
    CreateResponse {
        openagents: Some(options),
        ..ask(model)
    }
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

#[test]
fn a_model_id_expands_to_its_upstreams_credit_first() {
    let world = World::new();
    // Vercel keeps data, so strict privacy (the default) drops it.
    assert_eq!(
        world.route(&ask(FLASH)),
        pairs(&[("vertex", FLASH), ("openrouter", FLASH)])
    );
    let standard = with(
        FLASH,
        RequestOptions {
            privacy: Some(Privacy::Standard),
            ..RequestOptions::default()
        },
    );
    // With standard privacy Vercel is back, and cheaper than OpenRouter.
    assert_eq!(
        world.route(&standard),
        pairs(&[("vertex", FLASH), ("vercel", FLASH), ("openrouter", FLASH)])
    );
}

#[test]
fn a_class_without_scores_keeps_the_table_order() {
    let world = World::new();
    assert_eq!(
        world.route(&ask("openagents/fast")),
        pairs(&[("zai", GLM), ("vertex", FLASH), ("openrouter", FLASH)])
    );
    assert_eq!(
        world.route(&ask("openagents/chat")),
        pairs(&[
            ("vertex", FLASH),
            ("pro", "openai/gpt-5.6-terra"),
            ("openrouter", FLASH)
        ])
    );
    let plan = world.plan(&ask("openagents/chat")).unwrap();
    assert_eq!(plan.class, Some(TaskClass::Chat));
    assert_eq!(plan.first_token_ms, 8_000);
}

#[test]
fn with_scores_the_floor_filters_and_credit_ranks() {
    let mut world = World::new();
    world
        .classes
        .classes
        .get_mut(&TaskClass::Fast)
        .unwrap()
        .floor = Some(0.7);
    world.scores.by_class.insert(
        TaskClass::Fast,
        BTreeMap::from([(GLM.to_owned(), 0.75), (FLASH.to_owned(), 0.9)]),
    );
    // Both pass; credit first (both prepaid), the one expiring sooner (Z.ai,
    // 60 days) before Google (200 days), then cash.
    assert_eq!(
        world.route(&ask("openagents/fast")),
        pairs(&[("zai", GLM), ("vertex", FLASH), ("openrouter", FLASH)])
    );
    // Z.ai below the floor is no longer a candidate, credit or not.
    world
        .scores
        .by_class
        .get_mut(&TaskClass::Fast)
        .unwrap()
        .insert(GLM.into(), 0.5);
    let plan = world.plan(&ask("openagents/fast")).unwrap();
    assert_eq!(plan.attempts[0].upstream, "vertex");
    assert!(
        plan.dropped
            .iter()
            .any(|(c, why)| c.upstream == "zai" && *why == Dropped::BelowFloor)
    );
}

#[test]
fn the_request_structure_filters_capabilities() {
    let world = World::new();
    let mut request = ask("openagents/classify");
    request.tools = Some(vec![Tool::Function(FunctionTool {
        name: "f".into(),
        ..FunctionTool::default()
    })]);
    let plan = world.plan(&request).unwrap();
    // The Pro door takes no tools.
    assert_eq!(plan.attempts[0].upstream, "zai");
    assert!(
        plan.dropped
            .iter()
            .any(|(c, why)| c.upstream == "pro" && *why == Dropped::Capability("tools"))
    );

    // Past the Pro models' 400k context only the 1M models remain.
    let mut long = ask("openagents/reason");
    long.input = Some(Input::Text("x".repeat(2_000_000)));
    let plan = world.plan(&long).unwrap();
    assert!(plan.attempts.iter().all(|c| c.model == PRO));
}

#[test]
fn pay_mine_never_falls_back_to_ours() {
    let mut world = World::new();
    let mine = with(
        FLASH,
        RequestOptions {
            pay: Some(Payer::Mine),
            ..RequestOptions::default()
        },
    );
    let error = world.plan(&mine).unwrap_err();
    assert_eq!(error.kind, ErrorType::NoRoute);
    world.offerings.push(Offering {
        payer: Payer::Mine,
        ..offering("openrouter-byok", FLASH, true, true, None)
    });
    // The caller's own account needs no rate row of ours.
    assert_eq!(world.route(&mine), pairs(&[("openrouter-byok", FLASH)]));
}

#[test]
fn price_limits() {
    let mut world = World::new();
    let capped = with(
        PRO,
        RequestOptions {
            max_price: Some(MaxPrice {
                output: Some("5".into()),
                ..MaxPrice::default()
            }),
            ..RequestOptions::default()
        },
    );
    let error = world.plan(&capped).unwrap_err();
    assert_eq!(
        (error.status(), error.param.as_deref()),
        (403, Some("openagents.max_price"))
    );
    // The key owner's own limit applies the same way, margin included:
    // flash output is $2.50 + 5% = $2.625 per million.
    world.limits = PriceLimit {
        input: None,
        output: Some(2_600_000),
    };
    let plan = world
        .plan(&with(
            FLASH,
            RequestOptions {
                privacy: Some(Privacy::Standard),
                ..RequestOptions::default()
            },
        ))
        .unwrap();
    assert_eq!(
        plan.attempts
            .iter()
            .map(|c| c.upstream.as_str())
            .collect::<Vec<_>>(),
        ["vercel"]
    );
    let bad = with(
        FLASH,
        RequestOptions {
            max_price: Some(MaxPrice {
                input: Some("cheap".into()),
                ..MaxPrice::default()
            }),
            ..RequestOptions::default()
        },
    );
    assert_eq!(
        world.plan(&bad).unwrap_err().param.as_deref(),
        Some("openagents.max_price.input")
    );
}

#[test]
fn benched_upstreams_and_exhausted_credit() {
    let mut world = World::new();
    assert!(
        world
            .bench
            .observe("vertex", ErrorClass::Payment, NOW - 1_000)
    );
    assert!(!world.bench.observe("zai", ErrorClass::Server, NOW));
    assert_eq!(world.route(&ask(FLASH)), pairs(&[("openrouter", FLASH)]));
    assert!(!world.bench.is_benched("vertex", NOW - 1_000 + BENCH_MS));

    let mut world = World::new();
    world.ledger.set_balance("zai", 0);
    let plan = world.plan(&ask("openagents/fast")).unwrap();
    assert_eq!(plan.attempts[0].upstream, "vertex");
    assert!(
        plan.dropped
            .iter()
            .any(|(c, why)| c.upstream == "zai" && *why == Dropped::CreditExhausted)
    );
}

#[test]
fn caller_route_preferences() {
    let mut world = World::new();
    world.rates = vec![
        Rate {
            upstream: "openrouter".into(),
            model: FLASH.into(),
            ttft_p50_ms: Some(300),
            ..Rate::default()
        },
        Rate {
            upstream: "vertex".into(),
            model: FLASH.into(),
            ttft_p50_ms: Some(900),
            ..Rate::default()
        },
    ];
    let route = |route: RoutePreferences| {
        with(
            FLASH,
            RequestOptions {
                route: Some(route),
                ..RequestOptions::default()
            },
        )
    };
    assert_eq!(
        world.route(&route(RoutePreferences {
            sort: Some(Sort::Latency),
            ..RoutePreferences::default()
        })),
        pairs(&[("openrouter", FLASH), ("vertex", FLASH)])
    );
    assert_eq!(
        world.route(&route(RoutePreferences {
            order: vec!["openrouter".into()],
            ..RoutePreferences::default()
        })),
        pairs(&[("openrouter", FLASH), ("vertex", FLASH)])
    );
    assert_eq!(
        world.route(&route(RoutePreferences {
            ignore: vec!["vertex".into()],
            ..RoutePreferences::default()
        })),
        pairs(&[("openrouter", FLASH)])
    );
    assert_eq!(
        world.route(&route(RoutePreferences {
            only: vec!["vertex".into()],
            ..RoutePreferences::default()
        })),
        pairs(&[("vertex", FLASH)])
    );
}

#[test]
fn at_most_three_attempts_and_fallback_models() {
    let world = World::new();
    let request = with(
        "openagents/fast",
        RequestOptions {
            fallbacks: vec![PRO.into()],
            ..RequestOptions::default()
        },
    );
    let plan = world.plan(&request).unwrap();
    assert_eq!(plan.attempts.len(), 3);
    assert!(
        plan.dropped
            .iter()
            .any(|(c, why)| c.model == PRO && *why == Dropped::BeyondAttemptLimit)
    );
    // With the class's own entries gone, the fallback model is next.
    let mut world = World::new();
    world.offerings.retain(|o| o.model != FLASH);
    let plan = world.plan(&request).unwrap();
    assert_eq!(plan.attempts[1].model, PRO);
}

struct Says(TaskClass);

impl ClassJudge for Says {
    fn judge(&self, _: &CreateResponse) -> Option<TaskClass> {
        Some(self.0)
    }
}

#[test]
fn auto_takes_the_judged_class() {
    let world = World::new();
    let plan = world
        .plan_with(&ask("openagents/auto"), Some(&Says(TaskClass::Classify)))
        .unwrap();
    assert_eq!(plan.class, Some(TaskClass::Classify));
    assert_eq!(plan.attempts[0].model, "openai/gpt-5.6-luna");
    // No judgment available: chat.
    assert_eq!(
        world.plan(&ask("openagents/auto")).unwrap().class,
        Some(TaskClass::Chat)
    );
}

#[test]
fn unknown_models() {
    let world = World::new();
    for model in ["acme/nothing", "openagents/everything"] {
        let error = world.plan(&ask(model)).unwrap_err();
        assert_eq!(
            (error.status(), error.code.as_deref()),
            (404, Some("model_not_found")),
            "{model}"
        );
    }
    assert_eq!(
        world
            .plan(&CreateResponse::default())
            .unwrap_err()
            .param
            .as_deref(),
        Some("model")
    );
}

#[test]
fn fallback_only_before_the_first_token() {
    assert!(fall_back(false, ErrorClass::Server));
    assert!(fall_back(false, ErrorClass::FirstTokenDeadline));
    assert!(fall_back(false, ErrorClass::Payment));
    assert!(!fall_back(true, ErrorClass::StreamFailed));
    assert!(!fall_back(false, ErrorClass::BadRequest));
}

#[test]
fn money_strings() {
    assert_eq!(usd_micros("0.25"), Some(250_000));
    assert_eq!(usd_micros("3"), Some(3_000_000));
    assert_eq!(usd_micros(".0000005"), Some(1));
    assert_eq!(usd_micros("1e3"), None);
    assert_eq!(micros_usd(28), "0.000028");
    assert_eq!(micros_usd(2_500_000), "2.5");
    assert_eq!(micros_usd(0), "0");
}
