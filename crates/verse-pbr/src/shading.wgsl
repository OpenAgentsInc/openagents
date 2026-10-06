// Distances are meters. The authored profile preserves chamber content;
// the physical profile accepts candela. See lighting::PointProfile.
fn verse_point_falloff(distance_squared: f32, range: f32, physical: bool) -> f32 {
    if distance_squared >= range * range { return 0.0; }
    if physical {
        let d2 = max(distance_squared, 1e-4);
        let window = clamp(1.0 - pow(d2 / (range * range), 2.0), 0.0, 1.0);
        return window * window / (d2 + 0.01);
    }
    let window = max(1.0 - sqrt(distance_squared) / range, 0.0);
    return window * window / (1.0 + distance_squared);
}
// Opaque and retained masked fragments cover the surface completely.
fn verse_coverage(alpha: f32, blend: f32) -> f32 {
    return select(clamp(alpha, 0.0, 1.0), 1.0, blend < 2.0);
}
