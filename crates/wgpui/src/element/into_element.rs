use super::{AnyElement, Element};

pub trait IntoElement {
    type Element: Element;

    fn into_element(self) -> Self::Element;

    fn into_any_element(self) -> AnyElement
    where
        Self: Sized,
    {
        self.into_element().into_any()
    }
}

impl<T: Element> IntoElement for T {
    type Element = T;

    fn into_element(self) -> Self::Element {
        self
    }
}
