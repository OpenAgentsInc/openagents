use super::*;

fn flat(at: [f64; 3]) -> [f64; 2] {
    [at[0], at[2]]
}

fn apart(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// The spawn, the plaza's arches (Game Lab, Everglade, Lagrange 2, the
/// Gym's doorway side, Hosted), and the patrollers' routes, as
/// `verse::grid_robot::ROUTES` and the help desk's test list them.
const SPAWN: [f64; 2] = [0.0, -10.0];
const ARCHES: [[f64; 2]; 5] = [
    [-8.0, 3.0],
    [9.0, 1.0],
    [-20.0, 9.0],
    [0.0, 16.0],
    [20.0, 9.0],
];
const ROUTES: [[[f64; 2]; 2]; 4] = [
    [[13.0, 6.0], [13.0, 16.0]],
    [[-13.0, 16.0], [-13.0, 6.0]],
    [[16.0, -8.0], [8.0, -14.0]],
    [[-8.0, -14.0], [-16.0, -8.0]],
];
/// The dancer stands at most this far from the Everglade arch, m.
const DANCER_FROM_ARCH: f64 = 3.8;
/// A patroller's reach, m.
const ROBOT_RADIUS: f64 = 0.675;
const PLAYER_RADIUS: f64 = 0.45;

fn segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0);
    apart(p, [a[0] + dx * t, a[1] + dz * t])
}

#[test]
fn home_stands_beside_the_terminal_and_clear_of_the_plaza() {
    let home = flat(HOME);
    let body = f64::from(BODY_RADIUS);
    let terminal = flat(pylons_view::terminal::POSITION);
    // Beside the terminal, but its center outside the terminal's reach, so
    // a visitor walking up still opens the terminal.
    let to_terminal = apart(home, terminal);
    assert!(
        to_terminal > pylons_view::terminal::ENTER_RANGE,
        "{to_terminal}"
    );
    assert!(to_terminal < 4.0, "{to_terminal}");
    // Off the walk in from the spawn, which runs up x = 0: a visitor keeps
    // a clear lane past its body.
    assert!(home[0] - body - PLAYER_RADIUS > 0.9);
    assert!(apart(home, SPAWN) > 10.0);
    for arch in ARCHES {
        assert!(apart(home, arch) > 6.0, "{arch:?}");
    }
    // The dancer by the Everglade arch never meets it.
    assert!(apart(home, ARCHES[1]) - DANCER_FROM_ARCH > body + ROBOT_RADIUS);
    for [a, b] in ROUTES {
        assert!(segment_distance(home, a, b) > body + ROBOT_RADIUS + 2.0);
    }
    // The help desk (`grid_help_desk::POSITION`).
    let desk = [-6.0, -5.0];
    assert!(apart(home, desk) > 10.0);
    // The Pylon Bay's pad.
    let [u0, u1, v0, v1] = pylons_view::bay::PAD;
    let (a, b) = (
        pylons_view::bay::world(u0, v0),
        pylons_view::bay::world(u1, v1),
    );
    let x0 = a[0].min(b[0]);
    assert!(home[0] + body + 2.0 < x0, "{x0}");
}

#[test]
fn it_faces_the_terminal() {
    let yaw = home_yaw();
    let ahead = [HOME[0] + yaw.sin(), HOME[2] + yaw.cos()];
    assert!(apart(ahead, FACES) < apart(flat(HOME), FACES));
}

#[test]
fn it_is_twice_a_patroller() {
    assert!((SCALE - 2.0 * 1.5).abs() < f32::EPSILON);
    assert!((BODY_RADIUS - 1.35).abs() < 1e-6);
    assert!(TAG_LIFT > 5.6);
}

#[test]
fn status_lines_fit_and_count() {
    assert_eq!(status_line(14, 3), "14 OPEN ISSUES, 3 IN PROGRESS");
    assert_eq!(status_line(1, 0), "1 OPEN ISSUE, 0 IN PROGRESS");
    assert_eq!(status_line(3, 9), "3 OPEN ISSUES, 3 IN PROGRESS");
    assert_eq!(status_line(1234, 567), "1234 OPEN, 567 IN PROGRESS");
    for (open, busy) in [(0, 0), (99_999, 99_999), (u64::MAX, u64::MAX)] {
        let line = status_line(open, busy);
        assert!(line.len() <= STATUS_MAX, "{line}");
        assert!(line.chars().all(shown), "{line}");
    }
}

#[test]
fn clean_lines_keep_only_what_the_fonts_draw() {
    assert_eq!(clean_line("asking  flint…"), "ASKING FLINT");
    assert_eq!(clean_line("<script>x</script>"), "SCRIPT X /SCRIPT");
    assert_eq!(
        clean_line("reading fourteen open issues on the board today"),
        "READING FOURTEEN OPEN ISSUES ON"
    );
    assert_eq!(clean_line(&"A".repeat(50)).len(), STATUS_MAX);
    assert_eq!(clean_line(""), "");
}

#[test]
fn the_words_are_plain() {
    for text in [
        TAG,
        STATUS_WAITING,
        BACK_SOON,
        &status_line(14, 3),
        &status_line(1234, 567),
    ] {
        assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
    }
}

#[test]
fn the_pinned_key_is_a_public_key() {
    assert_eq!(PUBKEY.len(), 64);
    assert!(
        PUBKEY
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
}
