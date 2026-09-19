//! Measures how much a greedy flip rate moves when only the option order
//! moves.
//!
//! The flip rate this repository publishes is a greedy statistic over two
//! option orders: ask every Choice item in the suite's own order, ask it
//! again reversed, and count the items whose winning option changed. Greedy
//! decoding carries no seed, so the seed sweep in
//! `docs/lev/measurements/2026-09-19-seed-variance.md` says nothing about it.
//! Its perturbation axis is the option order itself.
//!
//! A three-option Choice has six orders and fifteen distinct pairs of them.
//! The published number is one of those fifteen. This sweep asks every item
//! under all six orders, so every pair's flip rate is computed rather than
//! estimated, and the spread over the fifteen is the whole population of
//! two-order flip rates this item set can produce.
//!
//! It runs against the bridge rather than through a door, for the same
//! reason `lev-seed-sweep` does: `lev-serve` serves the L2 estimator, and a
//! greedy call is a capability of Apple's runtime that no door publishes.
//! `gym permute` measures a door's argmax; this measures the statistic that
//! was published.
//!
//! Run it with the helper built:
//!
//! ```text
//! ./scripts/build-lev-bridge.sh
//! cargo run --release -p lev --bin lev-order-sweep -- \
//!     --door lev-base= \
//!     --door lev-choice=<path>/lev.fmadapter \
//!     --door lev-band=<path>/levband.fmadapter \
//!     --door lev-permutation=<path>/levperm.fmadapter \
//!     > docs/lev/measurements/<date>-flip-rate-variance.md
//! ```

use indexmap::IndexMap;
use lev::api::{Extensions, Question, SystemOneRequest};
use lev::bridge::{Call, Pool, Sampling};
use lev::schema::compile;
use lev::suite::{Item, Suite};

/// The suite the published flip rates were measured on.
const SUITE: &str = include_str!("../../suites/support-v2.json");

/// How many resamples an item-sampling interval is drawn from.
const RESAMPLES: usize = 10_000;

/// A door to sweep: a label and the adapter it serves through, if any.
struct Door {
    label: String,
    adapter: Option<String>,
}

/// What one door answered, item by item and order by order.
struct Answers {
    /// `[item][order]`, absent where the runtime refused the call.
    grid: Vec<Vec<Option<String>>>,
    /// A second pass in the suite's own order, which checks that a greedy
    /// call reproduces.
    repeat: Vec<Option<String>>,
    /// How many calls the runtime refused, and what it said about the first
    /// one. A sweep that quietly drops calls reports a flip rate over
    /// whatever survived, which is how a door that answers three items in
    /// ten looks perfectly stable.
    refused: usize,
    reason: Option<String>,
}

/// The mean, sample standard deviation, lowest, and highest of a series.
fn spread(values: &[f64]) -> (f64, f64, f64, f64) {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = if values.len() < 2 {
        0.0
    } else {
        values.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / (n - 1.0)
    };
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (mean, variance.sqrt(), low, high)
}

/// The population standard deviation of a series.
///
/// The fifteen order pairs are not a sample of the pairs that could have been
/// drawn. On a three-option suite they are all of them, so the spread over
/// them is a population and divides by `n`.
fn population_sd(values: &[f64]) -> f64 {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    (values.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / n).sqrt()
}

/// Every permutation of `0..count`, in lexicographic order.
fn orders(count: usize) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new()];
    for _ in 0..count {
        let mut next = Vec::new();
        for partial in &out {
            for index in 0..count {
                if !partial.contains(&index) {
                    let mut grown = partial.clone();
                    grown.push(index);
                    next.push(grown);
                }
            }
        }
        out = next;
    }
    out
}

/// The same Choice question with its options served in `order`.
fn permuted(question: &Question, order: &[usize]) -> Option<Question> {
    let Question::Choice { instructions, criteria } = question else {
        return None;
    };
    let keys: Vec<&String> = criteria.keys().collect();
    if order.len() != keys.len() {
        return None;
    }
    let mut reordered: IndexMap<String, Option<serde_json::Value>> = IndexMap::new();
    for index in order {
        let key = keys.get(*index)?;
        reordered.insert((*key).clone(), criteria.get(*key)?.clone());
    }
    Some(Question::Choice { instructions: instructions.clone(), criteria: reordered })
}

/// One greedy call for one item under one order.
fn call_for(item: &Item, question: &Question, adapter: Option<&str>) -> Option<Call> {
    let request = SystemOneRequest {
        state: item.state.clone(),
        model: None,
        questions: [("q".to_string(), question.clone())].into_iter().collect(),
        extensions: Extensions::default(),
    };
    let compiled = compile(&request).ok()?;
    let call = Call::decide(compiled.get("q")?, Sampling::Greedy);
    Some(match adapter {
        Some(path) => call.with_adapter(path),
        None => call,
    })
}

/// Asks one door every item under every order, plus a repeat of the first.
///
/// The pool is the door's own. A helper that has served one adapter and is
/// then asked to serve another keeps both resident, and a pool that carries
/// four adapters through a four-door sweep starts refusing calls partway
/// through — which reads as a door with no flips at all rather than as a
/// door that was never asked. One pool per door, started and stopped with
/// it, is what keeps that from being silently publishable.
fn sweep(pool: &Pool, door: &Door, items: &[&Item], orders: &[Vec<usize>]) -> Answers {
    let passes = orders.len() + 1;
    let mut calls = Vec::with_capacity(items.len() * passes);
    for item in items {
        for order in orders {
            let question = permuted(&item.question, order).expect("a Choice permutes");
            calls.push(call_for(item, &question, door.adapter.as_deref()).expect("a call builds"));
        }
        // The repeat pass asks the suite's own order a second time. Greedy
        // decoding should return the same option; measuring it is cheaper
        // than assuming it.
        let question = permuted(&item.question, &orders[0]).expect("a Choice permutes");
        calls.push(call_for(item, &question, door.adapter.as_deref()).expect("a call builds"));
    }

    let outcomes = pool.decide_all(&calls);
    let mut refused = 0;
    let mut reason: Option<String> = None;
    let mut answered = |outcome: &Result<lev::bridge::Outcome, lev::error::Refusal>| match outcome {
        Ok(outcome) => match outcome.choice.clone() {
            Some(choice) => Some(choice),
            None => {
                refused += 1;
                reason.get_or_insert_with(|| "the runtime selected no option".to_string());
                None
            }
        },
        Err(refusal) => {
            refused += 1;
            reason.get_or_insert_with(|| refusal.message.clone());
            None
        }
    };

    let mut grid = Vec::with_capacity(items.len());
    let mut repeat = Vec::with_capacity(items.len());
    for index in 0..items.len() {
        let base = index * passes;
        let row: Vec<Option<String>> =
            (0..orders.len()).map(|offset| answered(&outcomes[base + offset])).collect();
        grid.push(row);
        repeat.push(answered(&outcomes[base + orders.len()]));
    }
    if refused > 0 {
        eprintln!(
            "{}: the runtime refused {refused} of {} calls; first reason: {}",
            door.label,
            calls.len(),
            reason.clone().unwrap_or_default()
        );
    }
    Answers { grid, repeat, refused, reason }
}

/// The flip rate between two orders, and the items it was counted over.
fn flip_rate(answers: &Answers, left: usize, right: usize) -> (usize, usize) {
    let mut flips = 0;
    let mut trials = 0;
    for row in &answers.grid {
        let (Some(a), Some(b)) = (&row[left], &row[right]) else { continue };
        trials += 1;
        if a != b {
            flips += 1;
        }
    }
    (flips, trials)
}

/// Whether one item flipped between two orders, for a paired resample.
fn flipped(answers: &Answers, item: usize, left: usize, right: usize) -> Option<bool> {
    let row = &answers.grid[item];
    match (&row[left], &row[right]) {
        (Some(a), Some(b)) => Some(a != b),
        _ => None,
    }
}

/// A small deterministic generator, so a resampled interval reproduces.
struct Rolls(u64);

impl Rolls {
    fn next(&mut self) -> u64 {
        // xorshift64*, enough for drawing item indices.
        let mut state = self.0;
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        self.0 = state;
        state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn index(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

/// The standard deviation of a difference in flip rate under item resampling.
///
/// Both doors are resampled on the same items, because that is how the
/// comparison was measured: one suite, both doors.
fn resampled_difference(left: &Answers, right: &Answers, a: usize, b: usize, items: usize) -> f64 {
    let mut rolls = Rolls(0x5EED_1234_5678_9ABD);
    let mut differences = Vec::with_capacity(RESAMPLES);
    for _ in 0..RESAMPLES {
        let (mut left_flips, mut left_trials) = (0_f64, 0_f64);
        let (mut right_flips, mut right_trials) = (0_f64, 0_f64);
        for _ in 0..items {
            let item = rolls.index(items);
            if let Some(flip) = flipped(left, item, a, b) {
                left_flips += f64::from(u8::from(flip));
                left_trials += 1.0;
            }
            if let Some(flip) = flipped(right, item, a, b) {
                right_flips += f64::from(u8::from(flip));
                right_trials += 1.0;
            }
        }
        if left_trials == 0.0 || right_trials == 0.0 {
            continue;
        }
        differences.push(left_flips / left_trials - right_flips / right_trials);
    }
    let (_, sd, _, _) = spread(&differences);
    sd
}

/// The standard deviation, under item resampling, of a difference in the
/// flip rate averaged over every pair of orders.
///
/// Averaging over all fifteen pairs spends the whole order axis rather than
/// one draw from it, so what is left is the item axis alone. It is the
/// narrowest interval this suite can produce for a two-door comparison.
fn resampled_mean_difference(
    left: &Answers,
    right: &Answers,
    pairs: &[(usize, usize)],
    items: usize,
) -> f64 {
    let mut rolls = Rolls(0xABCD_5EED_0123_4567);
    let mut differences = Vec::with_capacity(RESAMPLES);
    for _ in 0..RESAMPLES {
        let drawn: Vec<usize> = (0..items).map(|_| rolls.index(items)).collect();
        let mut left_total = 0.0;
        let mut right_total = 0.0;
        for (a, b) in pairs {
            left_total += mean_flip(left, &drawn, *a, *b);
            right_total += mean_flip(right, &drawn, *a, *b);
        }
        differences.push((left_total - right_total) / pairs.len() as f64);
    }
    let (_, sd, _, _) = spread(&differences);
    sd
}

/// The flip rate between two orders over a drawn set of items.
fn mean_flip(answers: &Answers, drawn: &[usize], a: usize, b: usize) -> f64 {
    let mut flips = 0.0;
    let mut trials = 0.0;
    for item in drawn {
        if let Some(flip) = flipped(answers, *item, a, b) {
            flips += f64::from(u8::from(flip));
            trials += 1.0;
        }
    }
    if trials == 0.0 { 0.0 } else { flips / trials }
}

/// The standard deviation of one door's flip rate under item resampling.
fn resampled_rate(answers: &Answers, a: usize, b: usize, items: usize) -> f64 {
    let mut rolls = Rolls(0x1234_5EED_9ABC_DEF1);
    let mut rates = Vec::with_capacity(RESAMPLES);
    for _ in 0..RESAMPLES {
        let (mut flips, mut trials) = (0_f64, 0_f64);
        for _ in 0..items {
            let item = rolls.index(items);
            if let Some(flip) = flipped(answers, item, a, b) {
                flips += f64::from(u8::from(flip));
                trials += 1.0;
            }
        }
        if trials > 0.0 {
            rates.push(flips / trials);
        }
    }
    let (_, sd, _, _) = spread(&rates);
    sd
}

/// How often a door answers the same option under every order.
fn unanimous(answers: &Answers) -> (usize, usize) {
    let mut same = 0;
    let mut counted = 0;
    for row in &answers.grid {
        let seen: Vec<&String> = row.iter().flatten().collect();
        if seen.is_empty() {
            continue;
        }
        counted += 1;
        if seen.iter().all(|choice| *choice == seen[0]) {
            same += 1;
        }
    }
    (same, counted)
}

/// Accuracy under one order.
fn accuracy(answers: &Answers, items: &[&Item], order: usize) -> (usize, usize) {
    let mut correct = 0;
    let mut scored = 0;
    for (index, item) in items.iter().enumerate() {
        let Some(choice) = &answers.grid[index][order] else { continue };
        scored += 1;
        if choice == &item.truth {
            correct += 1;
        }
    }
    (correct, scored)
}

/// How an order reads in a table.
fn label(order: &[usize]) -> String {
    order.iter().map(usize::to_string).collect::<Vec<_>>().join("")
}

fn main() {
    let mut doors: Vec<Door> = Vec::new();
    let mut helpers = 4_usize;
    let mut limit = 0_usize;
    let mut rows: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--rows" => rows = args.next(),
            // A bounded run, for checking the instrument before spending an
            // hour of device time on it.
            "--items" => {
                limit = args.next().and_then(|value| value.parse().ok()).unwrap_or(limit);
            }
            "--door" => {
                let Some(spec) = args.next() else { continue };
                match spec.split_once('=') {
                    Some((label, path)) => doors.push(Door {
                        label: label.to_string(),
                        adapter: (!path.is_empty()).then(|| path.to_string()),
                    }),
                    None => eprintln!("--door takes label=adapter-path, got {spec}"),
                }
            }
            "--helpers" => {
                helpers = args.next().and_then(|value| value.parse().ok()).unwrap_or(helpers);
            }
            other => {
                eprintln!("unknown flag {other}");
                std::process::exit(2);
            }
        }
    }
    if doors.is_empty() {
        eprintln!("no doors; pass --door label=adapter-path, with an empty path for the base");
        std::process::exit(2);
    }

    let suite = Suite::load(SUITE).expect("the shipped suite loads");
    // The ten items the Gym's three-way suite locked are not read here. A
    // noise measurement is not worth spending a held-out partition on, and
    // the locked partition is spent through a ledger rather than by a sweep.
    let three_way = gym::suite::support_v2_three_way().expect("the committed suite loads");
    let locked: Vec<&str> = three_way
        .items
        .iter()
        .filter(|item| item.partition == gym::suite::Partition::Locked)
        .map(|item| item.id.as_str())
        .collect();

    let evaluation: Vec<&Item> = suite
        .split("evaluation")
        .filter(|item| matches!(item.question, Question::Choice { .. }))
        .collect();
    let readable: Vec<&Item> =
        evaluation.iter().copied().filter(|item| !locked.contains(&item.id.as_str())).collect();
    let width = match readable.first().map(|item| match &item.question {
        Question::Choice { criteria, .. } => criteria.len(),
        _ => 0,
    }) {
        Some(width) if width > 1 => width,
        _ => {
            eprintln!("the evaluation split holds no Choice item to permute");
            std::process::exit(2);
        }
    };
    let mut items: Vec<&Item> = readable
        .iter()
        .copied()
        .filter(|item| match &item.question {
            Question::Choice { criteria, .. } => criteria.len() == width,
            _ => false,
        })
        .collect();
    if limit > 0 {
        items.truncate(limit);
    }

    let every_order = orders(width);
    let reversed = every_order
        .iter()
        .position(|order| order.iter().copied().eq((0..width).rev()))
        .expect("the reversed order is a permutation");
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for left in 0..every_order.len() {
        for right in (left + 1)..every_order.len() {
            pairs.push((left, right));
        }
    }

    let pool = match Pool::discover(helpers) {
        Ok(pool) => pool,
        Err(refusal) => {
            eprintln!("no helper: {refusal}");
            std::process::exit(2);
        }
    };
    let availability = pool.availability().expect("availability answered");

    println!("# The spread a flip rate carries across option orders\n");
    println!(
        "Generated by `cargo run --release -p lev --bin lev-order-sweep`. Every number comes \
         from the live on-device runtime on one machine.\n"
    );
    println!("| Run fact | Value |");
    println!("| --- | --- |");
    println!("| Availability | `{}` |", availability.status);
    println!("| Suite | `{}`, digest `{}` |", suite.name, &suite.digest[..16]);
    println!("| Decoding | greedy, one call per item per order |");
    println!(
        "| Options per item | {width}, so {} orders and {} pairs |",
        every_order.len(),
        pairs.len()
    );
    println!(
        "| Items swept | {} of the {} Choice items in the evaluation split |",
        items.len(),
        evaluation.len()
    );
    println!(
        "| Items withheld | {} now in the three-way suite's locked partition |",
        evaluation.len() - readable.len()
    );
    println!("| Doors | {} |", doors.len());
    println!("| Pool width | {} |", pool.width());
    println!("| Calls | {} |\n", doors.len() * items.len() * (every_order.len() + 1));

    let swept: Vec<Answers> = doors
        .iter()
        .map(|door| {
            let own = match Pool::discover(helpers) {
                Ok(own) => own,
                Err(refusal) => {
                    eprintln!("no helper for {}: {refusal}", door.label);
                    std::process::exit(2);
                }
            };
            sweep(&own, door, &items, &every_order)
        })
        .collect();

    // Every answer the sweep drew, so the tables below can be recomputed, or
    // computed differently, without asking the runtime 1,120 more times.
    if let Some(path) = &rows {
        let mut lines = String::new();
        for (index, door) in doors.iter().enumerate() {
            for (position, item) in items.iter().enumerate() {
                for (order, permutation) in every_order.iter().enumerate() {
                    let answered = &swept[index].grid[position][order];
                    lines.push_str(&serde_json::json!({
                        "door": door.label,
                        "item": item.id,
                        "truth": item.truth,
                        "order": label(permutation),
                        "choice": answered,
                    }).to_string());
                    lines.push('\n');
                }
            }
        }
        match std::fs::write(path, lines) {
            Ok(()) => eprintln!("wrote the answers to {path}"),
            Err(trouble) => eprintln!("the answers were not written to {path}: {trouble}"),
        }
    }

    println!("## Every pair of orders\n");
    println!(
        "Each cell is the flip rate between two option orders: the share of items whose winning \
         option changed. Order `{}` is the suite's own and `{}` is the reversed one, so the pair \
         the record published is `{}` against `{}`.\n",
        label(&every_order[0]),
        label(&every_order[reversed]),
        label(&every_order[0]),
        label(&every_order[reversed])
    );
    print!("| Pair |");
    for door in &doors {
        print!(" `{}` |", door.label);
    }
    println!();
    print!("| --- |");
    for _ in &doors {
        print!(" --- |");
    }
    println!();
    let mut rates: Vec<Vec<f64>> = vec![Vec::new(); doors.len()];
    for (left, right) in &pairs {
        let published = (*left, *right) == (0, reversed);
        print!(
            "| `{}` against `{}`{} |",
            label(&every_order[*left]),
            label(&every_order[*right]),
            if published { " (published)" } else { "" }
        );
        for (index, answers) in swept.iter().enumerate() {
            let (flips, trials) = flip_rate(answers, *left, *right);
            let rate = flips as f64 / trials.max(1) as f64;
            rates[index].push(rate);
            print!(" {rate:.3} ({flips}/{trials}) |");
        }
        println!();
    }

    println!("\n## The spread per door\n");
    println!(
        "| Door | Mean | Standard deviation | Range | Published pair | Pairs below it | \
         Calls refused |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    let published_index = pairs.iter().position(|pair| *pair == (0, reversed)).expect("a pair");
    for (index, door) in doors.iter().enumerate() {
        let series = &rates[index];
        let (mean, _, low, high) = spread(series);
        let sd = population_sd(series);
        let published = series[published_index];
        let below = series.iter().filter(|rate| **rate < published).count();
        let refused = swept[index].refused;
        println!(
            "| `{}` | {mean:.3} | {sd:.4} | {low:.3} to {high:.3} | {published:.3} | \
             {below} of {} | {refused} of {} |",
            door.label,
            series.len(),
            items.len() * (every_order.len() + 1),
        );
    }
    for (index, door) in doors.iter().enumerate() {
        if let Some(reason) = &swept[index].reason {
            println!("\n`{}` refused first with: {reason}", door.label);
        }
    }

    println!("\n## Accuracy under each order\n");
    print!("| Order |");
    for door in &doors {
        print!(" `{}` |", door.label);
    }
    println!();
    print!("| --- |");
    for _ in &doors {
        print!(" --- |");
    }
    println!();
    let mut accuracies: Vec<Vec<f64>> = vec![Vec::new(); doors.len()];
    for (order, permutation) in every_order.iter().enumerate() {
        print!("| `{}` |", label(permutation));
        for (index, answers) in swept.iter().enumerate() {
            let (correct, scored) = accuracy(answers, &items, order);
            let rate = correct as f64 / scored.max(1) as f64;
            accuracies[index].push(rate);
            print!(" {rate:.3} |");
        }
        println!();
    }
    print!("| **Standard deviation** |");
    for series in &accuracies {
        print!(" {:.4} |", population_sd(series));
    }
    println!();

    println!("\n## What every order agrees on\n");
    println!("| Door | Unanimous across all orders | Repeat pass agrees |");
    println!("| --- | --- | --- |");
    for (index, door) in doors.iter().enumerate() {
        let (same, counted) = unanimous(&swept[index]);
        let agreed = swept[index]
            .grid
            .iter()
            .zip(&swept[index].repeat)
            .filter(|(row, again)| match (&row[0], again) {
                (Some(first), Some(again)) => first == again,
                _ => false,
            })
            .count();
        println!(
            "| `{}` | {same} of {counted} ({:.3}) | {agreed} of {} |",
            door.label,
            same as f64 / counted.max(1) as f64,
            items.len()
        );
    }

    println!("\n## What a comparison has to clear\n");
    println!(
        "The difference between two doors, measured on the same items, taken pair by pair. The \
         order axis and the item axis are separate: the first column is how much the difference \
         moves across the {} pairs of orders, and the second is how much it moves when the {} \
         items are resampled with replacement {RESAMPLES} times at the published pair.\n",
        pairs.len(),
        items.len()
    );
    println!(
        "| Comparison | At the published pair | Order-axis sd | Item-axis sd | Two sigma | \
         Reading | Mean over pairs | Two sigma over pairs | Reading |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for left in 0..doors.len() {
        for right in (left + 1)..doors.len() {
            let differences: Vec<f64> =
                (0..pairs.len()).map(|pair| rates[left][pair] - rates[right][pair]).collect();
            let (mean, _, _, _) = spread(&differences);
            let order_sd = population_sd(&differences);
            let (a, b) = pairs[published_index];
            let item_sd = resampled_difference(&swept[left], &swept[right], a, b, items.len());
            let combined = 2.0 * (order_sd.powi(2) + item_sd.powi(2)).sqrt();
            let published = differences[published_index];
            let reading = if published.abs() >= combined { "clears" } else { "inside the noise" };
            // The same comparison run the way it should be: every pair of
            // orders, which spends the order axis instead of drawing from it.
            let pooled_sd =
                resampled_mean_difference(&swept[left], &swept[right], &pairs, items.len());
            let pooled_reading =
                if mean.abs() >= 2.0 * pooled_sd { "clears" } else { "inside the noise" };
            println!(
                "| `{}` against `{}` | {published:+.3} | {order_sd:.4} | {item_sd:.4} | \
                 {combined:.3} | {reading} | {mean:+.3} | {:.3} | {pooled_reading} |",
                doors[left].label,
                doors[right].label,
                2.0 * pooled_sd
            );
        }
    }

    println!("\n## What one door's own number carries\n");
    println!("| Door | Published pair | Order-axis sd | Item-axis sd | Two sigma |");
    println!("| --- | --- | --- | --- | --- |");
    let (a, b) = pairs[published_index];
    for (index, door) in doors.iter().enumerate() {
        let order_sd = population_sd(&rates[index]);
        let item_sd = resampled_rate(&swept[index], a, b, items.len());
        println!(
            "| `{}` | {:.3} | {order_sd:.4} | {item_sd:.4} | {:.3} |",
            door.label,
            rates[index][published_index],
            2.0 * (order_sd.powi(2) + item_sd.powi(2)).sqrt()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn question(options: &[&str]) -> Question {
        let mut criteria: IndexMap<String, Option<serde_json::Value>> = IndexMap::new();
        for option in options {
            criteria.insert((*option).to_string(), None);
        }
        Question::Choice { instructions: Some(json!("which one")), criteria }
    }

    fn answers(rows: &[&[&str]]) -> Answers {
        Answers {
            grid: rows
                .iter()
                .map(|row| row.iter().map(|choice| Some((*choice).to_string())).collect())
                .collect(),
            repeat: Vec::new(),
            refused: 0,
            reason: None,
        }
    }

    #[test]
    fn every_order_of_three_options_is_listed_once() {
        let every = orders(3);
        assert_eq!(every.len(), 6);
        assert_eq!(every[0], vec![0, 1, 2], "the suite's own order comes first");
        for (index, order) in every.iter().enumerate() {
            assert!(!every[..index].contains(order), "{order:?} is listed twice");
        }
    }

    #[test]
    fn a_permutation_reorders_the_options_and_nothing_else() {
        let asked = permuted(&question(&["billing", "technical", "sales"]), &[2, 0, 1])
            .expect("a Choice permutes");
        let Question::Choice { instructions, criteria } = asked else {
            panic!("a Choice stays a Choice");
        };
        assert_eq!(instructions, Some(json!("which one")));
        assert_eq!(
            criteria.keys().cloned().collect::<Vec<String>>(),
            vec!["sales".to_string(), "billing".to_string(), "technical".to_string()]
        );
    }

    #[test]
    fn a_flip_is_an_item_that_answered_differently() {
        // Two items, three orders. The first item is unanimous; the second
        // answers `sales` under the third order alone.
        let measured = answers(&[
            &["billing", "billing", "billing"],
            &["technical", "technical", "sales"],
        ]);
        assert_eq!(flip_rate(&measured, 0, 1), (0, 2));
        assert_eq!(flip_rate(&measured, 0, 2), (1, 2));
        assert_eq!(unanimous(&measured), (1, 2));
    }

    #[test]
    fn an_unanswered_item_leaves_the_denominator_rather_than_counting_as_agreement() {
        let mut measured = answers(&[&["billing", "sales"], &["technical", "technical"]]);
        measured.grid[0][1] = None;
        assert_eq!(flip_rate(&measured, 0, 1), (0, 1), "the refused item is not counted");
        assert_eq!(flipped(&measured, 0, 0, 1), None);
    }

    #[test]
    fn the_population_spread_divides_by_every_value() {
        // Four values whose mean is 2.5 and whose squared deviations sum to 5.
        let values = [1.0, 2.0, 3.0, 4.0];
        assert!((population_sd(&values) - 1.118_033_988_749_895).abs() < 1e-12);
        let (mean, sample_sd, low, high) = spread(&values);
        assert!((mean - 2.5).abs() < 1e-12);
        assert!(sample_sd > population_sd(&values), "a sample spread is the wider one");
        assert!((low - 1.0).abs() < 1e-12 && (high - 4.0).abs() < 1e-12);
    }
}
