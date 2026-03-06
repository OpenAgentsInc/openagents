use wgpui::markdown::MarkdownDocument;

#[derive(Clone, Debug)]
pub struct Deck {
    pub metadata: DeckMetadata,
    pub slides: Vec<Slide>,
}

impl Deck {
    pub fn is_empty(&self) -> bool {
        self.slides.is_empty()
    }

    pub fn slide_count(&self) -> usize {
        self.slides.len()
    }
}

#[derive(Clone, Debug)]
pub struct DeckMetadata {
    pub title: String,
    pub slug: Option<String>,
    pub theme: DeckTheme,
}

#[derive(Clone, Debug)]
pub struct Slide {
    pub id: String,
    pub title: String,
    pub eyebrow: Option<String>,
    pub summary: Option<String>,
    pub footer: Option<String>,
    pub sources: Vec<String>,
    pub theme: DeckTheme,
    pub layout: SlideLayout,
    pub diagram: Option<SlideDiagram>,
    pub notes: Option<String>,
    pub transition: SlideTransition,
    pub kind: SlideKind,
}

impl Slide {
    pub fn markdown(&self) -> Option<&MarkdownSlide> {
        match &self.kind {
            SlideKind::Markdown(slide) => Some(slide),
        }
    }
}

#[derive(Clone, Debug)]
pub enum SlideKind {
    Markdown(MarkdownSlide),
}

#[derive(Clone, Debug)]
pub struct MarkdownSlide {
    pub markdown: String,
    pub document: MarkdownDocument,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeckTheme {
    Hud,
    Minimal,
    Code,
    Diagram,
    Custom(String),
}

impl DeckTheme {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "hud" => Self::Hud,
            "minimal" => Self::Minimal,
            "code" => Self::Code,
            "diagram" => Self::Diagram,
            other => Self::Custom(other.to_string()),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Hud => "hud",
            Self::Minimal => "minimal",
            Self::Code => "code",
            Self::Diagram => "diagram",
            Self::Custom(label) => label.as_str(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlideDiagram {
    MarketMap,
    ComputeFlow,
    AccessGrant,
    ContractChain,
    LiquidityRoute,
    RiskLoop,
}

impl SlideDiagram {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "market-map" | "market_map" | "market map" => Ok(Self::MarketMap),
            "compute-flow" | "compute_flow" | "compute flow" => Ok(Self::ComputeFlow),
            "access-grant" | "access_grant" | "access grant" => Ok(Self::AccessGrant),
            "contract-chain" | "contract_chain" | "contract chain" => Ok(Self::ContractChain),
            "liquidity-route" | "liquidity_route" | "liquidity route" => Ok(Self::LiquidityRoute),
            "risk-loop" | "risk_loop" | "risk loop" => Ok(Self::RiskLoop),
            other => Err(format!("unsupported slide diagram '{other}'")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlideLayout {
    Title,
    Body,
    TwoColumn,
    Code,
}

impl SlideLayout {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "title" => Ok(Self::Title),
            "body" => Ok(Self::Body),
            "two-column" | "two_column" | "two column" => Ok(Self::TwoColumn),
            "code" => Ok(Self::Code),
            other => Err(format!("unsupported slide layout '{other}'")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlideTransition {
    None,
    Fade,
}

impl SlideTransition {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "fade" => Ok(Self::Fade),
            other => Err(format!("unsupported slide transition '{other}'")),
        }
    }
}
