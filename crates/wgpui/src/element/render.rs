use crate::app::{App, Context};

use super::IntoElement;

pub trait Render: 'static + Sized {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement;
}

pub trait RenderOnce: 'static {
    fn render(self, cx: &mut App) -> impl IntoElement;
}
