//! A bounded harbor mask, baked into the water texture array. Its small
//! radial record is shared by gameplay sampling and both renderers.

use glam::Vec2;

/// Swell gain is `gain` through `inner` meters, then joins open water at
/// `outer`. Detail ripples, rain, and tides remain independent of shelter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shelter {
    pub center: [f32; 2],
    pub inner: f32,
    pub outer: f32,
    pub gain: f32,
}

impl Shelter {
    pub fn valid(self) -> bool {
        self.center.iter().all(|x| x.is_finite())
            && self.inner.is_finite()
            && self.inner >= 0.0
            && self.outer.is_finite()
            && self.outer > self.inner
            && self.gain.is_finite()
            && (0.0..=1.0).contains(&self.gain)
    }

    /// Gain and its x/z gradient. The gradient contributes `height *
    /// gradient` to the sheltered surface's normal through the transition.
    pub fn sample(self, p: Vec2) -> [f32; 3] {
        let offset = p - Vec2::from(self.center);
        let radius = offset.length();
        let t = ((radius - self.inner) / (self.outer - self.inner)).clamp(0.0, 1.0);
        let gain = self.gain + (1.0 - self.gain) * t * t * (3.0 - 2.0 * t);
        let derivative = (1.0 - self.gain) * 6.0 * t * (1.0 - t) / (self.outer - self.inner);
        let gradient = offset / radius.max(1e-6) * derivative;
        [gain, gradient.x, gradient.y]
    }

    /// Add an open-water border so repeat sampling cannot join opposite
    /// edges of the circular mask. One array layer needs no extra sampler.
    pub fn bounds(self) -> ([f32; 2], f32) {
        let half = self.outer * 1.04;
        ([self.center[0] - half, self.center[1] - half], half * 2.0)
    }

    pub fn bake(self, size: usize) -> Vec<[u16; 4]> {
        let (origin, side) = self.bounds();
        (0..size * size)
            .map(|index| {
                let p = Vec2::from(origin)
                    + Vec2::new((index % size) as f32 + 0.5, (index / size) as f32 + 0.5)
                        * (side / size as f32);
                let [gain, dx, dz] = self.sample(p);
                [gain, dx, dz, 0.0].map(super::ocean::half)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_preserves_open_water_and_matches_the_height_gradient() {
        let mask = Shelter {
            center: [-130.0, -230.0],
            inner: 55.0,
            outer: 95.0,
            gain: 0.1,
        };
        assert!(mask.valid());
        assert_eq!(mask.sample(Vec2::from(mask.center)), [0.1, 0.0, 0.0]);
        assert_eq!(mask.sample(Vec2::ZERO), [1.0, 0.0, 0.0]);
        let p = Vec2::from(mask.center) + Vec2::new(65.0, 20.0);
        let value = mask.sample(p);
        for (axis, expected) in [(Vec2::X, value[1]), (Vec2::Y, value[2])] {
            let derivative =
                (mask.sample(p + axis * 0.01)[0] - mask.sample(p - axis * 0.01)[0]) / 0.02;
            assert!((derivative - expected).abs() < 0.0001);
        }
        for size in [64, 128] {
            let texels = mask.bake(size);
            assert_eq!(texels.len(), size * size);
            for x in 0..size {
                assert_eq!(texels[x][0], super::super::ocean::half(1.0));
                assert_eq!(
                    texels[(size - 1) * size + x][0],
                    super::super::ocean::half(1.0)
                );
            }
            assert_eq!(texels, mask.bake(size));
        }
        assert!(
            !Shelter {
                outer: mask.inner,
                ..mask
            }
            .valid()
        );
        assert!(
            !Shelter {
                gain: f32::NAN,
                ..mask
            }
            .valid()
        );
    }
}
