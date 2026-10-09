//! The table family: container, scroller, table, header cells, rows and
//! cells, with numeric columns and column sizes.

use maud::{Markup, Render, html};

/// The minimum width of a column, so a wide table scrolls instead of
/// squeezing its prose columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColSize {
    /// 6rem.
    Sm,
    /// 10rem.
    Md,
    /// 16rem.
    Lg,
}

impl ColSize {
    fn attr(self) -> &'static str {
        match self {
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
        }
    }
}

/// A data table:
/// `div.oa-table-container > div.oa-table-scroller > table.oa-table`.
/// A table wider than its column scrolls sideways inside the scroller,
/// which is focusable so the keyboard can scroll it too.
#[derive(Clone, Debug, Default)]
pub struct Table {
    label: Option<String>,
    caption: Option<Markup>,
    header: Vec<Markup>,
    rows: Vec<Vec<Markup>>,
    numeric: Vec<usize>,
    sizes: Vec<(usize, ColSize)>,
}

impl Table {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The scroll region's accessible name (default: "Table").
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A caption (text is escaped).
    #[must_use]
    pub fn caption(mut self, caption: impl Render) -> Self {
        self.caption = Some(caption.render());
        self
    }

    /// The header cells (text is escaped).
    #[must_use]
    pub fn header<I, R>(mut self, cells: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: Render,
    {
        self.header = cells.into_iter().map(|c| c.render()).collect();
        self
    }

    /// Appends a body row (text is escaped).
    #[must_use]
    pub fn row<I, R>(mut self, cells: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: Render,
    {
        self.rows
            .push(cells.into_iter().map(|c| c.render()).collect());
        self
    }

    /// Marks column `index` (from 0) numeric: end-aligned, tabular figures.
    #[must_use]
    pub fn numeric(mut self, index: usize) -> Self {
        self.numeric.push(index);
        self
    }

    /// Sets the minimum width of column `index` (from 0).
    #[must_use]
    pub fn col_size(mut self, index: usize, size: ColSize) -> Self {
        self.sizes.retain(|(i, _)| *i != index);
        self.sizes.push((index, size));
        self
    }

    fn numeric_at(&self, index: usize) -> bool {
        self.numeric.contains(&index)
    }

    fn size_at(&self, index: usize) -> Option<&'static str> {
        self.sizes
            .iter()
            .find(|(i, _)| *i == index)
            .map(|(_, s)| s.attr())
    }
}

impl Render for Table {
    fn render(&self) -> Markup {
        let label = self.label.as_deref().unwrap_or("Table");
        html! {
            div.oa-table-container {
                div.oa-table-scroller tabindex="0" role="region" aria-label=(label) {
                    table.oa-table {
                        @if let Some(caption) = &self.caption { caption { (caption) } }
                        @if !self.header.is_empty() {
                            thead {
                                tr {
                                    @for (i, cell) in self.header.iter().enumerate() {
                                        th scope="col" data-numeric[self.numeric_at(i)] data-col-size=[self.size_at(i)] { (cell) }
                                    }
                                }
                            }
                        }
                        tbody {
                            @for row in &self.rows {
                                tr {
                                    @for (i, cell) in row.iter().enumerate() {
                                        td data-numeric[self.numeric_at(i)] data-col-size=[self.size_at(i)] { (cell) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_scrolls_in_its_container_and_escapes_cells() {
        let html = Table::new()
            .label("Prices")
            .header(["Item", "Price"])
            .row(["<b>Tea</b>", "3.50"])
            .numeric(1)
            .col_size(0, ColSize::Sm)
            .col_size(0, ColSize::Md)
            .render()
            .into_string();
        assert!(html.starts_with(
            "<div class=\"oa-table-container\"><div class=\"oa-table-scroller\" tabindex=\"0\" role=\"region\" aria-label=\"Prices\"><table class=\"oa-table\">"
        ));
        assert!(
            html.contains("<th scope=\"col\" data-col-size=\"md\">Item</th>"),
            "{html}"
        );
        assert!(
            html.contains("<th scope=\"col\" data-numeric>Price</th>"),
            "{html}"
        );
        assert!(
            html.contains("<td data-col-size=\"md\">&lt;b&gt;Tea&lt;/b&gt;</td>"),
            "{html}"
        );
        assert!(html.contains("<td data-numeric>3.50</td>"), "{html}");
    }

    #[test]
    fn table_without_header_has_no_thead() {
        let html = Table::new().row(["a"]).render().into_string();
        assert!(!html.contains("<thead>"), "{html}");
        assert!(html.contains("aria-label=\"Table\""), "{html}");
    }
}
