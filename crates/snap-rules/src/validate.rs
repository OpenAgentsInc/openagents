//! Structural checks and a replayed reference solution for each shipped job.

use crate::{Game, Input, PieceKind, Status, bot, build, level::Level, receipt::Receipt};

#[derive(Clone, Debug)]
pub struct Checked {
    pub pieces: u32,
    pub inputs: usize,
    pub ticks: u32,
    pub receipt: Receipt,
}

/// Checks bounds and inventory, then requires a three-star bot run and replay.
pub fn check(level: &Level) -> Result<Checked, String> {
    if level.name.is_empty() || level.name.len() > 64 || level.number == 0 {
        return Err("give the job a name and number".into());
    }
    let (low, high) = level.area;
    if low.x >= high.x || low.y >= high.y || high.x - low.x > 64. || high.y - low.y > 64. {
        return Err("the build area is too large or empty".into());
    }
    if level.run == 0
        || level.run > 60 * crate::HZ
        || level.tests.is_empty()
        || level.tests.len() > 8
    {
        return Err("the job needs one to eight tests, each under a minute".into());
    }
    if level.given.is_empty()
        || level.given.len() > 64
        || level.tray.iter().map(|(_, n)| *n).sum::<u32>() > 256
    {
        return Err("too many pieces".into());
    }
    let mut tags = Vec::new();
    for g in &level.given {
        if let Some(tag) = g.tag {
            if tags.contains(&tag) {
                return Err("two pieces have the same tag".into());
            }
            tags.push(tag);
        }
    }
    for (i, (k, n)) in level.tray.iter().enumerate() {
        if *n == 0 || *n > 99 || level.tray[..i].iter().any(|(other, _)| other == k) {
            return Err("each tray piece needs one count".into());
        }
    }
    let built = level.start();
    for (i, p) in built.pieces.iter().enumerate() {
        if p.at
            .iter()
            .any(|v| v.x < low.x || v.x > high.x || v.y < low.y || v.y > high.y)
        {
            return Err("a starting piece is outside the build area".into());
        }
        if built.pieces[..i]
            .iter()
            .any(|q| build::overlaps(&p.at, &q.at))
        {
            return Err("starting pieces overlap".into());
        }
    }
    if level.tests.iter().any(|j| matches!(j, crate::Job::Spin(_)))
        && !level.given.iter().any(|g| g.kind == PieceKind::Pod)
    {
        return Err("a garden needs plant pods".into());
    }
    if level
        .required
        .iter()
        .any(|r| r.count == 0 || r.count > level.tray_count(r.kind))
    {
        return Err("the tray is missing a required piece".into());
    }
    let plan = bot::solve(level).ok_or("the bot could not finish this job")?;
    let (game, receipt) = bot::record_plan(level, &plan);
    finish(level, &game, &plan, &receipt)?;
    Ok(Checked {
        pieces: game.build.used(),
        inputs: receipt.inputs.len(),
        ticks: game.tick,
        receipt,
    })
}

fn finish(level: &Level, game: &Game, plan: &[Input], receipt: &Receipt) -> Result<(), String> {
    if game.status != Status::Won || game.stars != 3 || game.build.used() != level.par {
        return Err(format!(
            "the reference build needs {} pieces; par is {}",
            game.build.used(),
            level.par
        ));
    }
    if plan.len() > 512 || receipt.inputs.len() > 520 {
        return Err("the reference build has too many steps".into());
    }
    if game
        .world
        .as_ref()
        .is_some_and(|w| w.pos.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()))
    {
        return Err("the run could not finish".into());
    }
    receipt.verify(level)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shipped_jobs_have_real_three_star_runs_and_matching_replays() {
        for n in 1..=crate::level::COUNT {
            let l = crate::level::job(n);
            let checked = check(&l).unwrap_or_else(|e| panic!("job {n}: {e}"));
            assert!(checked.inputs > 0);
            assert!(checked.pieces > 0);
        }
    }
    #[test]
    fn bad_layouts_and_inventories_do_not_reach_the_bot() {
        let mut l = crate::level::job(1);
        l.given[0].centre.x = -99.;
        assert!(check(&l).unwrap_err().contains("outside"));
        let mut l = crate::level::job(1);
        l.given[1].centre = l.given[0].centre;
        assert!(check(&l).unwrap_err().contains("overlap"));
        let mut l = crate::level::job(1);
        let mut wrong_par = crate::level::job(1);
        wrong_par.par = 999;
        assert!(check(&wrong_par).unwrap_err().contains("par"));
        l.required.push(crate::level::Required {
            kind: PieceKind::Hinge,
            count: 1,
            tag: None,
        });
        assert!(check(&l).unwrap_err().contains("missing"));
    }
}
