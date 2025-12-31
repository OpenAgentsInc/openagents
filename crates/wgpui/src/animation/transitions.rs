use std::time::Duration;

use super::{Animatable, Animation, AnimationState, Easing, Keyframe, KeyframeAnimation};

/// A transition animation for enter/exit states.
#[derive(Debug, Clone)]
pub struct Transition<T: Animatable> {
    pub entering: TransitionAnimation<T>,
    pub exiting: TransitionAnimation<T>,
}

/// Transition animation variants supported by the builder helpers.
#[derive(Debug, Clone)]
pub enum TransitionAnimation<T: Animatable> {
    Tween(Animation<T>),
    Keyframes(KeyframeAnimation<T>),
}

impl<T: Animatable> TransitionAnimation<T> {
    pub fn start(&mut self) {
        match self {
            TransitionAnimation::Tween(animation) => animation.start(),
            TransitionAnimation::Keyframes(animation) => animation.start(),
        }
    }

    pub fn tick(&mut self, delta: Duration) -> T {
        match self {
            TransitionAnimation::Tween(animation) => animation.tick(delta),
            TransitionAnimation::Keyframes(animation) => animation.tick(delta),
        }
    }

    pub fn state(&self) -> AnimationState {
        match self {
            TransitionAnimation::Tween(animation) => animation.state(),
            TransitionAnimation::Keyframes(animation) => animation.state(),
        }
    }

    pub fn is_finished(&self) -> bool {
        match self {
            TransitionAnimation::Tween(animation) => animation.is_finished(),
            TransitionAnimation::Keyframes(animation) => animation.is_finished(),
        }
    }
}

/// Build a fade transition (opacity 0 -> 1).
pub fn fade(duration: Duration) -> Transition<f32> {
    Transition {
        entering: TransitionAnimation::Tween(
            Animation::new(0.0, 1.0, duration).easing(Easing::EaseInOut),
        ),
        exiting: TransitionAnimation::Tween(
            Animation::new(1.0, 0.0, duration).easing(Easing::EaseInOut),
        ),
    }
}

/// Build a flicker transition with a sine easing curve.
pub fn flicker(duration: Duration) -> Transition<f32> {
    let entering_keyframes = vec![
        Keyframe::new(0.0_f32, 0.0),
        Keyframe::new(1.0, 0.33).easing(Easing::EaseOutSine),
        Keyframe::new(0.5, 0.66).easing(Easing::EaseOutSine),
        Keyframe::new(1.0, 1.0).easing(Easing::EaseOutSine),
    ];

    let exiting_keyframes = vec![
        Keyframe::new(1.0_f32, 0.0),
        Keyframe::new(0.0, 0.33).easing(Easing::EaseOutSine),
        Keyframe::new(0.5, 0.66).easing(Easing::EaseOutSine),
        Keyframe::new(0.0, 1.0).easing(Easing::EaseOutSine),
    ];

    Transition {
        entering: TransitionAnimation::Keyframes(KeyframeAnimation::new(
            entering_keyframes,
            duration,
        )),
        exiting: TransitionAnimation::Keyframes(KeyframeAnimation::new(
            exiting_keyframes,
            duration,
        )),
    }
}

/// Build a draw transition for SVG path stroke-dashoffset animation.
pub fn draw(path_length: f32, duration: Duration, easing: Option<Easing>) -> Transition<f32> {
    let length = path_length.max(0.0);
    let easing = easing.unwrap_or(Easing::EaseInOut);

    Transition {
        entering: TransitionAnimation::Tween(Animation::new(length, 0.0, duration).easing(easing)),
        exiting: TransitionAnimation::Tween(Animation::new(0.0, length, duration).easing(easing)),
    }
}

/// Build a basic transition between two values.
pub fn transition<T: Animatable>(
    from: T,
    to: T,
    duration: Duration,
    easing: Easing,
    back: Option<T>,
) -> Transition<T> {
    let exit_target = back.unwrap_or(from);
    Transition {
        entering: TransitionAnimation::Tween(Animation::new(from, to, duration).easing(easing)),
        exiting: TransitionAnimation::Tween(
            Animation::new(to, exit_target, duration).easing(easing),
        ),
    }
}
