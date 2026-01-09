//! DSPy Signatures for Agent Orchestrator subagents.
//!
//! Contains learnable signatures for:
//! - ArchitectureSignature (Oracle) - CoT for design decisions
//! - LibraryLookupSignature (Librarian)
//! - CodeExplorationSignature (Explore)
//! - UIDesignSignature (Frontend)
//! - DocumentationSignature (DocWriter)
//! - MediaAnalysisSignature (Multimodal)

use anyhow::Result;
use dsrs::core::signature::MetaSignature;
use dsrs::data::example::Example;
use serde_json::{json, Value};

// ============================================================================
// ArchitectureSignature (Oracle) - Chain-of-Thought
// ============================================================================

/// Architecture decision complexity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchitectureComplexity {
    /// Simple change, minimal impact.
    Low,
    /// Moderate change, affects multiple components.
    Medium,
    /// Major change, system-wide impact.
    High,
    /// Critical change, requires careful review.
    Critical,
}

impl std::fmt::Display for ArchitectureComplexity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchitectureComplexity::Low => write!(f, "LOW"),
            ArchitectureComplexity::Medium => write!(f, "MEDIUM"),
            ArchitectureComplexity::High => write!(f, "HIGH"),
            ArchitectureComplexity::Critical => write!(f, "CRITICAL"),
        }
    }
}

impl std::str::FromStr for ArchitectureComplexity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "LOW" => Ok(ArchitectureComplexity::Low),
            "MEDIUM" => Ok(ArchitectureComplexity::Medium),
            "HIGH" => Ok(ArchitectureComplexity::High),
            "CRITICAL" => Ok(ArchitectureComplexity::Critical),
            _ => Err(format!("Unknown complexity: {}", s)),
        }
    }
}

/// Architecture Signature for Oracle.
///
/// Provides architecture advice with chain-of-thought reasoning.
/// Used for complex design decisions, debugging after multiple failures,
/// and security analysis.
///
/// # Inputs
/// - `requirements`: What needs to be designed or decided
/// - `existing_architecture`: Current system state (JSON)
/// - `constraints`: Technical and business constraints
///
/// # Outputs (CoT)
/// - `reasoning`: Step-by-step architectural thinking
/// - `proposed_changes`: Recommended changes with rationale
/// - `tradeoffs`: Pros and cons of the approach
/// - `risks`: Potential issues and mitigations
/// - `complexity`: LOW, MEDIUM, HIGH, CRITICAL
#[derive(Debug, Clone)]
pub struct ArchitectureSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for ArchitectureSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are Oracle, an architecture advisor. Analyze the requirements and provide architectural guidance.

Think step-by-step through:
1. What are the core requirements?
2. How does this fit the existing architecture?
3. What patterns or approaches could work?
4. What are the tradeoffs of each approach?
5. What are the risks and how to mitigate them?

Consider:
- Maintainability and technical debt
- Performance implications
- Security concerns
- Scalability requirements
- Testing strategy
- Backward compatibility

Provide clear, actionable recommendations with reasoning for each decision."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl ArchitectureSignature {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for ArchitectureSignature {
    fn demos(&self) -> Vec<Example> {
        self.demos.clone()
    }

    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> {
        self.demos = demos;
        Ok(())
    }

    fn instruction(&self) -> String {
        self.instruction.clone()
    }

    fn input_fields(&self) -> Value {
        json!({
            "requirements": {
                "type": "String",
                "desc": "What needs to be designed or decided",
                "__dsrs_field_type": "input"
            },
            "existing_architecture": {
                "type": "String",
                "desc": "Current system state as JSON (components, dependencies, patterns)",
                "__dsrs_field_type": "input"
            },
            "constraints": {
                "type": "String",
                "desc": "Technical and business constraints",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "reasoning": {
                "type": "String",
                "desc": "Step-by-step architectural thinking (chain-of-thought)",
                "__dsrs_field_type": "output"
            },
            "proposed_changes": {
                "type": "String",
                "desc": "Recommended changes with rationale",
                "__dsrs_field_type": "output"
            },
            "tradeoffs": {
                "type": "String",
                "desc": "Pros and cons of the proposed approach",
                "__dsrs_field_type": "output"
            },
            "risks": {
                "type": "String",
                "desc": "Potential issues and suggested mitigations",
                "__dsrs_field_type": "output"
            },
            "complexity": {
                "type": "String",
                "desc": "Change complexity: LOW, MEDIUM, HIGH, CRITICAL",
                "__dsrs_field_type": "output"
            }
        })
    }

    fn update_instruction(&mut self, instruction: String) -> Result<()> {
        self.instruction = instruction;
        Ok(())
    }

    fn append(&mut self, _name: &str, _value: Value) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// LibraryLookupSignature (Librarian)
// ============================================================================

/// Library Lookup Signature for Librarian.
///
/// Finds documentation and usage examples for external libraries.
/// Specializes in GitHub search, OSS reference, and API documentation.
///
/// # Inputs
/// - `query`: What to look up
/// - `library_name`: Specific library (optional)
/// - `context`: What the user is trying to accomplish
///
/// # Outputs
/// - `findings`: Relevant documentation and examples
/// - `sources`: Where the information came from
/// - `code_examples`: Runnable code snippets
/// - `confidence`: How reliable the information is (0.0-1.0)
#[derive(Debug, Clone)]
pub struct LibraryLookupSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for LibraryLookupSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are Librarian, an expert in external documentation and library usage.

Your role:
- Find relevant documentation for libraries and APIs
- Provide accurate, up-to-date usage examples
- Reference official sources when possible
- Identify best practices and common patterns
- Note any version-specific considerations

When searching:
1. Prioritize official documentation
2. Look for working code examples
3. Check for known issues or deprecations
4. Consider the user's specific use case
5. Rate confidence based on source reliability

Always cite your sources and indicate confidence level."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl LibraryLookupSignature {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for LibraryLookupSignature {
    fn demos(&self) -> Vec<Example> {
        self.demos.clone()
    }

    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> {
        self.demos = demos;
        Ok(())
    }

    fn instruction(&self) -> String {
        self.instruction.clone()
    }

    fn input_fields(&self) -> Value {
        json!({
            "query": {
                "type": "String",
                "desc": "What to look up - the question or topic",
                "__dsrs_field_type": "input"
            },
            "library_name": {
                "type": "String",
                "desc": "Specific library name (optional, can be empty)",
                "__dsrs_field_type": "input"
            },
            "context": {
                "type": "String",
                "desc": "What the user is trying to accomplish",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "findings": {
                "type": "String",
                "desc": "Relevant documentation and explanations",
                "__dsrs_field_type": "output"
            },
            "sources": {
                "type": "String",
                "desc": "URLs and references where information was found",
                "__dsrs_field_type": "output"
            },
            "code_examples": {
                "type": "String",
                "desc": "Runnable code snippets demonstrating usage",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in the information (0.0-1.0)",
                "__dsrs_field_type": "output"
            }
        })
    }

    fn update_instruction(&mut self, instruction: String) -> Result<()> {
        self.instruction = instruction;
        Ok(())
    }

    fn append(&mut self, _name: &str, _value: Value) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// CodeExplorationSignature (Explore)
// ============================================================================

/// Search type for code exploration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchType {
    /// Find where something is defined.
    Definition,
    /// Find all references to something.
    References,
    /// Search for a pattern in code.
    Pattern,
    /// Find usage examples.
    Usage,
    /// Trace call graph.
    CallGraph,
}

impl std::fmt::Display for SearchType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchType::Definition => write!(f, "definition"),
            SearchType::References => write!(f, "references"),
            SearchType::Pattern => write!(f, "pattern"),
            SearchType::Usage => write!(f, "usage"),
            SearchType::CallGraph => write!(f, "call_graph"),
        }
    }
}

impl std::str::FromStr for SearchType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "definition" | "def" => Ok(SearchType::Definition),
            "references" | "refs" => Ok(SearchType::References),
            "pattern" => Ok(SearchType::Pattern),
            "usage" => Ok(SearchType::Usage),
            "call_graph" | "callgraph" | "calls" => Ok(SearchType::CallGraph),
            _ => Err(format!("Unknown search type: {}", s)),
        }
    }
}

/// Code Exploration Signature for Explore.
///
/// Navigates codebase to find relevant code quickly.
/// Optimized for fast pattern matching and navigation.
///
/// # Inputs
/// - `query`: What to find (function, pattern, concept)
/// - `search_type`: definition, references, pattern, usage, call_graph
/// - `scope`: Files/directories to search (empty for all)
///
/// # Outputs
/// - `locations`: File:line locations found
/// - `code_snippets`: Relevant code with context
/// - `related_files`: Other files worth checking
/// - `confidence`: Match quality (0.0-1.0)
#[derive(Debug, Clone)]
pub struct CodeExplorationSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for CodeExplorationSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are Explore, a fast codebase navigator.

Your role:
- Find code quickly and accurately
- Locate definitions, references, and patterns
- Navigate complex codebases efficiently
- Identify related files and dependencies

Search strategies:
1. For definitions: Use AST-aware search, check exports
2. For references: Grep with context, follow imports
3. For patterns: Regex with file type filtering
4. For usage: Find call sites, check tests
5. For call graphs: Trace function calls up/down

Provide:
- Exact file:line locations
- Relevant code snippets with context
- Confidence rating for each match
- Related files that might be relevant"#
                .to_string(),
            demos: vec![],
        }
    }
}

impl CodeExplorationSignature {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for CodeExplorationSignature {
    fn demos(&self) -> Vec<Example> {
        self.demos.clone()
    }

    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> {
        self.demos = demos;
        Ok(())
    }

    fn instruction(&self) -> String {
        self.instruction.clone()
    }

    fn input_fields(&self) -> Value {
        json!({
            "query": {
                "type": "String",
                "desc": "What to find - function name, pattern, or concept",
                "__dsrs_field_type": "input"
            },
            "search_type": {
                "type": "String",
                "desc": "Search type: definition, references, pattern, usage, call_graph",
                "__dsrs_field_type": "input"
            },
            "scope": {
                "type": "String",
                "desc": "Files/directories to search (empty for entire codebase)",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "locations": {
                "type": "String",
                "desc": "File:line locations found (one per line)",
                "__dsrs_field_type": "output"
            },
            "code_snippets": {
                "type": "String",
                "desc": "Relevant code with surrounding context",
                "__dsrs_field_type": "output"
            },
            "related_files": {
                "type": "String",
                "desc": "Other files worth checking (imports, tests, etc.)",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Match quality (0.0-1.0)",
                "__dsrs_field_type": "output"
            }
        })
    }

    fn update_instruction(&mut self, instruction: String) -> Result<()> {
        self.instruction = instruction;
        Ok(())
    }

    fn append(&mut self, _name: &str, _value: Value) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// UIDesignSignature (Frontend)
// ============================================================================

/// UI Design Signature for Frontend.
///
/// Designs visual elements and styling for user interfaces.
/// Specializes in CSS, Tailwind, accessibility, and responsive design.
///
/// # Inputs
/// - `design_request`: What UI to create or modify
/// - `existing_styles`: Current design system (JSON)
/// - `constraints`: Accessibility, responsive, browser requirements
///
/// # Outputs
/// - `css_changes`: CSS/Tailwind classes to apply
/// - `component_structure`: HTML/JSX structure
/// - `design_rationale`: Why these choices were made
/// - `accessibility_notes`: A11y considerations and compliance
#[derive(Debug, Clone)]
pub struct UIDesignSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for UIDesignSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are Frontend, a UI/UX specialist.

Your role:
- Design visual elements and layouts
- Create and modify CSS/Tailwind styles
- Ensure accessibility compliance
- Optimize for responsive design

Design principles:
1. Match existing design system patterns
2. Prioritize accessibility (WCAG 2.1 AA)
3. Mobile-first responsive design
4. Performance-conscious (minimize reflows)
5. Consistent spacing and typography

Output:
- Specific CSS classes or Tailwind utilities
- Component HTML/JSX structure
- Reasoning for design decisions
- Accessibility considerations and ARIA attributes"#
                .to_string(),
            demos: vec![],
        }
    }
}

impl UIDesignSignature {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for UIDesignSignature {
    fn demos(&self) -> Vec<Example> {
        self.demos.clone()
    }

    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> {
        self.demos = demos;
        Ok(())
    }

    fn instruction(&self) -> String {
        self.instruction.clone()
    }

    fn input_fields(&self) -> Value {
        json!({
            "design_request": {
                "type": "String",
                "desc": "What UI to create or modify",
                "__dsrs_field_type": "input"
            },
            "existing_styles": {
                "type": "String",
                "desc": "Current design system as JSON (colors, spacing, typography)",
                "__dsrs_field_type": "input"
            },
            "constraints": {
                "type": "String",
                "desc": "Accessibility, responsive, browser requirements",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "css_changes": {
                "type": "String",
                "desc": "CSS properties or Tailwind classes to apply",
                "__dsrs_field_type": "output"
            },
            "component_structure": {
                "type": "String",
                "desc": "HTML/JSX structure for the component",
                "__dsrs_field_type": "output"
            },
            "design_rationale": {
                "type": "String",
                "desc": "Reasoning for design choices",
                "__dsrs_field_type": "output"
            },
            "accessibility_notes": {
                "type": "String",
                "desc": "A11y considerations, ARIA attributes, keyboard navigation",
                "__dsrs_field_type": "output"
            }
        })
    }

    fn update_instruction(&mut self, instruction: String) -> Result<()> {
        self.instruction = instruction;
        Ok(())
    }

    fn append(&mut self, _name: &str, _value: Value) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// DocumentationSignature (DocWriter)
// ============================================================================

/// Documentation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    /// README file.
    Readme,
    /// API reference documentation.
    ApiRef,
    /// Tutorial or guide.
    Guide,
    /// Code comments.
    Comment,
    /// Changelog entry.
    Changelog,
}

impl std::fmt::Display for DocType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocType::Readme => write!(f, "readme"),
            DocType::ApiRef => write!(f, "api"),
            DocType::Guide => write!(f, "guide"),
            DocType::Comment => write!(f, "comment"),
            DocType::Changelog => write!(f, "changelog"),
        }
    }
}

impl std::str::FromStr for DocType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "readme" => Ok(DocType::Readme),
            "api" | "apiref" | "api_ref" => Ok(DocType::ApiRef),
            "guide" | "tutorial" => Ok(DocType::Guide),
            "comment" | "comments" => Ok(DocType::Comment),
            "changelog" | "changes" => Ok(DocType::Changelog),
            _ => Err(format!("Unknown doc type: {}", s)),
        }
    }
}

/// Documentation Signature for DocWriter.
///
/// Writes technical documentation of various types.
/// Specializes in clear, accurate, and well-structured docs.
///
/// # Inputs
/// - `doc_type`: readme, api, guide, comment, changelog
/// - `subject`: What to document
/// - `audience`: Who will read it (developers, users, etc.)
/// - `existing_docs`: Current documentation to reference
///
/// # Outputs
/// - `content`: The documentation text
/// - `structure`: Section breakdown (for longer docs)
/// - `examples`: Code/usage examples to include
/// - `cross_references`: Related docs to link
#[derive(Debug, Clone)]
pub struct DocumentationSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for DocumentationSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are DocWriter, a technical documentation specialist.

Your role:
- Write clear, accurate documentation
- Match existing documentation style
- Include practical examples
- Structure content logically

Documentation principles:
1. Start with the "why" before the "how"
2. Use consistent terminology
3. Include runnable code examples
4. Add cross-references to related topics
5. Keep it concise but complete

For different doc types:
- README: Quick start, installation, basic usage
- API: Parameters, return values, examples
- Guide: Step-by-step with explanations
- Comment: Concise, explains "why" not "what"
- Changelog: User-facing changes, grouped by type"#
                .to_string(),
            demos: vec![],
        }
    }
}

impl DocumentationSignature {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for DocumentationSignature {
    fn demos(&self) -> Vec<Example> {
        self.demos.clone()
    }

    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> {
        self.demos = demos;
        Ok(())
    }

    fn instruction(&self) -> String {
        self.instruction.clone()
    }

    fn input_fields(&self) -> Value {
        json!({
            "doc_type": {
                "type": "String",
                "desc": "Documentation type: readme, api, guide, comment, changelog",
                "__dsrs_field_type": "input"
            },
            "subject": {
                "type": "String",
                "desc": "What to document",
                "__dsrs_field_type": "input"
            },
            "audience": {
                "type": "String",
                "desc": "Target audience (developers, users, maintainers)",
                "__dsrs_field_type": "input"
            },
            "existing_docs": {
                "type": "String",
                "desc": "Current documentation to reference or extend",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "content": {
                "type": "String",
                "desc": "The documentation text (markdown)",
                "__dsrs_field_type": "output"
            },
            "structure": {
                "type": "String",
                "desc": "Section breakdown for longer documents",
                "__dsrs_field_type": "output"
            },
            "examples": {
                "type": "String",
                "desc": "Code/usage examples to include",
                "__dsrs_field_type": "output"
            },
            "cross_references": {
                "type": "String",
                "desc": "Related documentation to link",
                "__dsrs_field_type": "output"
            }
        })
    }

    fn update_instruction(&mut self, instruction: String) -> Result<()> {
        self.instruction = instruction;
        Ok(())
    }

    fn append(&mut self, _name: &str, _value: Value) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// MediaAnalysisSignature (Multimodal)
// ============================================================================

/// Media type for analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    /// Static image (PNG, JPG, etc.).
    Image,
    /// PDF document.
    Pdf,
    /// Diagram or flowchart.
    Diagram,
    /// Screenshot of UI or terminal.
    Screenshot,
    /// Video frame or animation.
    Video,
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaType::Image => write!(f, "image"),
            MediaType::Pdf => write!(f, "pdf"),
            MediaType::Diagram => write!(f, "diagram"),
            MediaType::Screenshot => write!(f, "screenshot"),
            MediaType::Video => write!(f, "video"),
        }
    }
}

impl std::str::FromStr for MediaType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "image" | "img" | "picture" => Ok(MediaType::Image),
            "pdf" | "document" => Ok(MediaType::Pdf),
            "diagram" | "flowchart" | "chart" => Ok(MediaType::Diagram),
            "screenshot" | "screen" => Ok(MediaType::Screenshot),
            "video" | "animation" | "gif" => Ok(MediaType::Video),
            _ => Err(format!("Unknown media type: {}", s)),
        }
    }
}

/// Media Analysis Signature for Multimodal.
///
/// Interprets visual content including images, PDFs, and diagrams.
/// Extracts structured information from visual inputs.
///
/// # Inputs
/// - `media_type`: image, pdf, diagram, screenshot, video
/// - `content_description`: Brief description of what the media shows
/// - `analysis_focus`: What specific information to extract
///
/// # Outputs
/// - `description`: Detailed description of media contents
/// - `extracted_data`: Key information pulled from the media
/// - `structured_output`: JSON if applicable (tables, data)
/// - `uncertainties`: What's unclear or ambiguous
#[derive(Debug, Clone)]
pub struct MediaAnalysisSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for MediaAnalysisSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are Multimodal, a media analysis specialist.

Your role:
- Analyze images, PDFs, diagrams, and screenshots
- Extract structured information from visual content
- Describe visual elements accurately
- Identify text, data, and relationships

Analysis approach:
1. Identify the media type and format
2. Describe overall structure and layout
3. Extract text and data systematically
4. Note visual elements (colors, shapes, arrows)
5. Identify relationships and hierarchy
6. Flag any uncertainties or ambiguities

Output:
- Detailed natural language description
- Extracted data in structured format
- JSON for tabular or structured content
- Confidence indicators for uncertain elements"#
                .to_string(),
            demos: vec![],
        }
    }
}

impl MediaAnalysisSignature {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for MediaAnalysisSignature {
    fn demos(&self) -> Vec<Example> {
        self.demos.clone()
    }

    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> {
        self.demos = demos;
        Ok(())
    }

    fn instruction(&self) -> String {
        self.instruction.clone()
    }

    fn input_fields(&self) -> Value {
        json!({
            "media_type": {
                "type": "String",
                "desc": "Media type: image, pdf, diagram, screenshot, video",
                "__dsrs_field_type": "input"
            },
            "content_description": {
                "type": "String",
                "desc": "Brief description of what the media shows",
                "__dsrs_field_type": "input"
            },
            "analysis_focus": {
                "type": "String",
                "desc": "What specific information to extract",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "description": {
                "type": "String",
                "desc": "Detailed description of media contents",
                "__dsrs_field_type": "output"
            },
            "extracted_data": {
                "type": "String",
                "desc": "Key information pulled from the media",
                "__dsrs_field_type": "output"
            },
            "structured_output": {
                "type": "String",
                "desc": "JSON representation of structured content (tables, data)",
                "__dsrs_field_type": "output"
            },
            "uncertainties": {
                "type": "String",
                "desc": "What's unclear, ambiguous, or low confidence",
                "__dsrs_field_type": "output"
            }
        })
    }

    fn update_instruction(&mut self, instruction: String) -> Result<()> {
        self.instruction = instruction;
        Ok(())
    }

    fn append(&mut self, _name: &str, _value: Value) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architecture_signature_fields() {
        let sig = ArchitectureSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("requirements").is_some());
        assert!(inputs.get("existing_architecture").is_some());
        assert!(inputs.get("constraints").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("reasoning").is_some()); // CoT
        assert!(outputs.get("proposed_changes").is_some());
        assert!(outputs.get("tradeoffs").is_some());
        assert!(outputs.get("risks").is_some());
        assert!(outputs.get("complexity").is_some());
    }

    #[test]
    fn test_library_lookup_signature_fields() {
        let sig = LibraryLookupSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("query").is_some());
        assert!(inputs.get("library_name").is_some());
        assert!(inputs.get("context").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("findings").is_some());
        assert!(outputs.get("sources").is_some());
        assert!(outputs.get("code_examples").is_some());
        assert!(outputs.get("confidence").is_some());
    }

    #[test]
    fn test_code_exploration_signature_fields() {
        let sig = CodeExplorationSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("query").is_some());
        assert!(inputs.get("search_type").is_some());
        assert!(inputs.get("scope").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("locations").is_some());
        assert!(outputs.get("code_snippets").is_some());
        assert!(outputs.get("related_files").is_some());
        assert!(outputs.get("confidence").is_some());
    }

    #[test]
    fn test_ui_design_signature_fields() {
        let sig = UIDesignSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("design_request").is_some());
        assert!(inputs.get("existing_styles").is_some());
        assert!(inputs.get("constraints").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("css_changes").is_some());
        assert!(outputs.get("component_structure").is_some());
        assert!(outputs.get("design_rationale").is_some());
        assert!(outputs.get("accessibility_notes").is_some());
    }

    #[test]
    fn test_documentation_signature_fields() {
        let sig = DocumentationSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("doc_type").is_some());
        assert!(inputs.get("subject").is_some());
        assert!(inputs.get("audience").is_some());
        assert!(inputs.get("existing_docs").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("content").is_some());
        assert!(outputs.get("structure").is_some());
        assert!(outputs.get("examples").is_some());
        assert!(outputs.get("cross_references").is_some());
    }

    #[test]
    fn test_media_analysis_signature_fields() {
        let sig = MediaAnalysisSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("media_type").is_some());
        assert!(inputs.get("content_description").is_some());
        assert!(inputs.get("analysis_focus").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("description").is_some());
        assert!(outputs.get("extracted_data").is_some());
        assert!(outputs.get("structured_output").is_some());
        assert!(outputs.get("uncertainties").is_some());
    }

    #[test]
    fn test_search_type_parse() {
        assert_eq!("definition".parse::<SearchType>().unwrap(), SearchType::Definition);
        assert_eq!("refs".parse::<SearchType>().unwrap(), SearchType::References);
        assert_eq!("pattern".parse::<SearchType>().unwrap(), SearchType::Pattern);
        assert_eq!("usage".parse::<SearchType>().unwrap(), SearchType::Usage);
        assert_eq!("call_graph".parse::<SearchType>().unwrap(), SearchType::CallGraph);
    }

    #[test]
    fn test_doc_type_parse() {
        assert_eq!("readme".parse::<DocType>().unwrap(), DocType::Readme);
        assert_eq!("api".parse::<DocType>().unwrap(), DocType::ApiRef);
        assert_eq!("guide".parse::<DocType>().unwrap(), DocType::Guide);
        assert_eq!("comment".parse::<DocType>().unwrap(), DocType::Comment);
        assert_eq!("changelog".parse::<DocType>().unwrap(), DocType::Changelog);
    }

    #[test]
    fn test_media_type_parse() {
        assert_eq!("image".parse::<MediaType>().unwrap(), MediaType::Image);
        assert_eq!("pdf".parse::<MediaType>().unwrap(), MediaType::Pdf);
        assert_eq!("diagram".parse::<MediaType>().unwrap(), MediaType::Diagram);
        assert_eq!("screenshot".parse::<MediaType>().unwrap(), MediaType::Screenshot);
        assert_eq!("video".parse::<MediaType>().unwrap(), MediaType::Video);
    }

    #[test]
    fn test_architecture_complexity_parse() {
        assert_eq!("LOW".parse::<ArchitectureComplexity>().unwrap(), ArchitectureComplexity::Low);
        assert_eq!("medium".parse::<ArchitectureComplexity>().unwrap(), ArchitectureComplexity::Medium);
        assert_eq!("High".parse::<ArchitectureComplexity>().unwrap(), ArchitectureComplexity::High);
        assert_eq!("CRITICAL".parse::<ArchitectureComplexity>().unwrap(), ArchitectureComplexity::Critical);
    }
}
