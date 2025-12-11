//! Core types for the Vibe IDE screen
//!
//! Defines all data structures for projects, files, database, deployments,
//! and the various UI states needed for the full Vibe experience.

use std::collections::HashMap;

// ============================================================================
// Tab Navigation
// ============================================================================

/// Main navigation tabs in Vibe
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VibeTab {
    #[default]
    Projects,
    Editor,
    Database,
    Deploy,
}

impl VibeTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Projects => "PROJECTS",
            Self::Editor => "EDITOR",
            Self::Database => "DATABASE",
            Self::Deploy => "DEPLOY",
        }
    }

    pub fn shortcut(&self) -> &'static str {
        match self {
            Self::Projects => "1",
            Self::Editor => "2",
            Self::Database => "3",
            Self::Deploy => "4",
        }
    }

    pub fn all() -> &'static [VibeTab] {
        &[Self::Projects, Self::Editor, Self::Database, Self::Deploy]
    }
}

// ============================================================================
// Project Types
// ============================================================================

/// Project status
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProjectStatus {
    #[default]
    Active,
    Building,
    Error,
    Deployed,
    Archived,
}

impl ProjectStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Building => "BUILDING",
            Self::Error => "ERROR",
            Self::Deployed => "LIVE",
            Self::Archived => "ARCHIVED",
        }
    }

    pub fn indicator(&self) -> &'static str {
        match self {
            Self::Active => "●",
            Self::Building => "◐",
            Self::Error => "✕",
            Self::Deployed => "▲",
            Self::Archived => "○",
        }
    }
}

/// Project framework/template type
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProjectFramework {
    #[default]
    React,
    NextJs,
    Vue,
    Svelte,
    Astro,
    Vanilla,
    Custom,
}

impl ProjectFramework {
    pub fn label(&self) -> &'static str {
        match self {
            Self::React => "React",
            Self::NextJs => "Next.js",
            Self::Vue => "Vue",
            Self::Svelte => "Svelte",
            Self::Astro => "Astro",
            Self::Vanilla => "Vanilla",
            Self::Custom => "Custom",
        }
    }
}

/// A Vibe project
#[derive(Clone, Debug)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub framework: ProjectFramework,
    pub status: ProjectStatus,
    pub created_at: String,
    pub updated_at: String,
    pub file_count: u32,
    pub deployed_url: Option<String>,
    pub custom_domain: Option<String>,
    pub has_database: bool,
    pub has_auth: bool,
    pub has_payments: bool,
    pub collaborator_count: u32,
    pub agent_credits_used: u32,
}

impl Project {
    pub fn mock(id: &str, name: &str, framework: ProjectFramework, status: ProjectStatus) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: format!("A {} project built with Vibe", framework.label()),
            framework,
            status,
            created_at: "2024-12-01".to_string(),
            updated_at: "2024-12-10".to_string(),
            file_count: 47,
            deployed_url: if status == ProjectStatus::Deployed {
                Some(format!("{}.vibe.dev", id))
            } else {
                None
            },
            custom_domain: None,
            has_database: true,
            has_auth: true,
            has_payments: false,
            collaborator_count: 2,
            agent_credits_used: 156,
        }
    }
}

/// Generate mock projects for UI development
pub fn mock_projects() -> Vec<Project> {
    vec![
        {
            let mut p = Project::mock("saas-dashboard", "SaaS Dashboard", ProjectFramework::React, ProjectStatus::Deployed);
            p.description = "Analytics dashboard with charts, user management, and billing".to_string();
            p.file_count = 89;
            p.has_payments = true;
            p.custom_domain = Some("dashboard.mycompany.com".to_string());
            p.collaborator_count = 4;
            p
        },
        {
            let mut p = Project::mock("landing-page", "Landing Page", ProjectFramework::Astro, ProjectStatus::Deployed);
            p.description = "Marketing site with blog, pricing, and contact form".to_string();
            p.file_count = 34;
            p.has_database = false;
            p.has_auth = false;
            p
        },
        {
            let mut p = Project::mock("ecommerce-store", "E-Commerce Store", ProjectFramework::NextJs, ProjectStatus::Building);
            p.description = "Full shop with cart, checkout, and inventory management".to_string();
            p.file_count = 156;
            p.has_payments = true;
            p
        },
        {
            let mut p = Project::mock("blog-cms", "Blog CMS", ProjectFramework::React, ProjectStatus::Active);
            p.description = "Content management system with markdown editor".to_string();
            p.file_count = 62;
            p
        },
        {
            let mut p = Project::mock("api-playground", "API Playground", ProjectFramework::Vanilla, ProjectStatus::Active);
            p.description = "Interactive API documentation and testing tool".to_string();
            p.file_count = 28;
            p.has_auth = false;
            p
        },
        {
            let mut p = Project::mock("task-tracker", "Task Tracker", ProjectFramework::Vue, ProjectStatus::Error);
            p.description = "Kanban board with team collaboration features".to_string();
            p.file_count = 71;
            p.collaborator_count = 3;
            p
        },
    ]
}

// ============================================================================
// Template Types
// ============================================================================

/// Template category
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TemplateCategory {
    #[default]
    All,
    SaaS,
    Ecommerce,
    Marketing,
    Portfolio,
    Dashboard,
    Mobile,
    Api,
    Internal,
}

impl TemplateCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::SaaS => "SAAS",
            Self::Ecommerce => "ECOMMERCE",
            Self::Marketing => "MARKETING",
            Self::Portfolio => "PORTFOLIO",
            Self::Dashboard => "DASHBOARD",
            Self::Mobile => "MOBILE",
            Self::Api => "API",
            Self::Internal => "INTERNAL",
        }
    }

    pub fn all() -> &'static [TemplateCategory] {
        &[
            Self::All,
            Self::SaaS,
            Self::Ecommerce,
            Self::Marketing,
            Self::Portfolio,
            Self::Dashboard,
            Self::Mobile,
            Self::Api,
            Self::Internal,
        ]
    }
}

/// A project template
#[derive(Clone, Debug)]
pub struct ProjectTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: TemplateCategory,
    pub framework: ProjectFramework,
    pub features: Vec<String>,
    pub use_count: u64,
    pub preview_url: Option<String>,
}

impl ProjectTemplate {
    pub fn mock(id: &str, name: &str, category: TemplateCategory, framework: ProjectFramework) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: format!("Start with a {} template", name.to_lowercase()),
            category,
            framework,
            features: vec!["Auth".to_string(), "Database".to_string()],
            use_count: 12_500,
            preview_url: Some(format!("https://templates.vibe.dev/{}", id)),
        }
    }
}

/// Generate mock templates
pub fn mock_templates() -> Vec<ProjectTemplate> {
    vec![
        {
            let mut t = ProjectTemplate::mock("saas-starter", "SaaS Starter", TemplateCategory::SaaS, ProjectFramework::NextJs);
            t.description = "Complete SaaS foundation with auth, billing, and team management".to_string();
            t.features = vec!["Auth".to_string(), "Subscriptions".to_string(), "Teams".to_string(), "Email".to_string()];
            t.use_count = 45_200;
            t
        },
        {
            let mut t = ProjectTemplate::mock("ecommerce-pro", "E-Commerce Pro", TemplateCategory::Ecommerce, ProjectFramework::NextJs);
            t.description = "Full-featured store with cart, checkout, and inventory".to_string();
            t.features = vec!["Payments".to_string(), "Inventory".to_string(), "Reviews".to_string(), "Search".to_string()];
            t.use_count = 38_100;
            t
        },
        {
            let mut t = ProjectTemplate::mock("landing-kit", "Landing Kit", TemplateCategory::Marketing, ProjectFramework::Astro);
            t.description = "High-converting landing page with A/B testing".to_string();
            t.features = vec!["Analytics".to_string(), "Forms".to_string(), "SEO".to_string()];
            t.use_count = 67_800;
            t
        },
        {
            let mut t = ProjectTemplate::mock("admin-dashboard", "Admin Dashboard", TemplateCategory::Dashboard, ProjectFramework::React);
            t.description = "Data-rich admin interface with charts and tables".to_string();
            t.features = vec!["Charts".to_string(), "Tables".to_string(), "RBAC".to_string(), "Export".to_string()];
            t.use_count = 29_400;
            t
        },
        {
            let mut t = ProjectTemplate::mock("portfolio-dev", "Developer Portfolio", TemplateCategory::Portfolio, ProjectFramework::Astro);
            t.description = "Showcase your work with projects and blog".to_string();
            t.features = vec!["Blog".to_string(), "Projects".to_string(), "Contact".to_string()];
            t.use_count = 52_100;
            t
        },
        {
            let mut t = ProjectTemplate::mock("api-service", "API Service", TemplateCategory::Api, ProjectFramework::Custom);
            t.description = "REST/GraphQL API with auth and rate limiting".to_string();
            t.features = vec!["Auth".to_string(), "Rate Limit".to_string(), "Docs".to_string(), "Monitoring".to_string()];
            t.use_count = 18_900;
            t
        },
        {
            let mut t = ProjectTemplate::mock("internal-tool", "Internal Tool", TemplateCategory::Internal, ProjectFramework::React);
            t.description = "Quick internal tools for your team".to_string();
            t.features = vec!["Auth".to_string(), "Database".to_string(), "Forms".to_string()];
            t.use_count = 14_200;
            t
        },
        {
            let mut t = ProjectTemplate::mock("blank", "Blank Project", TemplateCategory::All, ProjectFramework::React);
            t.description = "Start from scratch with just the essentials".to_string();
            t.features = vec![];
            t.use_count = 89_500;
            t
        },
    ]
}

// ============================================================================
// File System Types (OANIX Namespace)
// ============================================================================

/// File type for syntax highlighting and icons
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileType {
    #[default]
    Unknown,
    TypeScript,
    JavaScript,
    Rust,
    Html,
    Css,
    Json,
    Markdown,
    Image,
    Config,
    Directory,
}

impl FileType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "ts" | "tsx" => Self::TypeScript,
            "js" | "jsx" | "mjs" => Self::JavaScript,
            "rs" => Self::Rust,
            "html" | "htm" => Self::Html,
            "css" | "scss" | "sass" => Self::Css,
            "json" => Self::Json,
            "md" | "mdx" => Self::Markdown,
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => Self::Image,
            "toml" | "yaml" | "yml" | "env" => Self::Config,
            _ => Self::Unknown,
        }
    }

    pub fn indicator(&self) -> &'static str {
        match self {
            Self::Unknown => " ",
            Self::TypeScript => "TS",
            Self::JavaScript => "JS",
            Self::Rust => "RS",
            Self::Html => "HT",
            Self::Css => "CS",
            Self::Json => "{}",
            Self::Markdown => "MD",
            Self::Image => "IM",
            Self::Config => "CF",
            Self::Directory => "/",
        }
    }
}

/// Git status for a file
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GitStatus {
    #[default]
    Unchanged,
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

impl GitStatus {
    pub fn indicator(&self) -> &'static str {
        match self {
            Self::Unchanged => " ",
            Self::Modified => "M",
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Untracked => "?",
            Self::Conflicted => "!",
        }
    }
}

/// A file or directory in the project
#[derive(Clone, Debug)]
pub struct ProjectFile {
    pub path: String,
    pub name: String,
    pub file_type: FileType,
    pub is_directory: bool,
    pub size_bytes: u64,
    pub git_status: GitStatus,
    pub children: Vec<ProjectFile>,
    pub is_expanded: bool,
    pub depth: u32,
}

impl ProjectFile {
    pub fn file(path: &str, name: &str, file_type: FileType, size: u64, git_status: GitStatus) -> Self {
        Self {
            path: path.to_string(),
            name: name.to_string(),
            file_type,
            is_directory: false,
            size_bytes: size,
            git_status,
            children: vec![],
            is_expanded: false,
            depth: path.matches('/').count() as u32,
        }
    }

    pub fn directory(path: &str, name: &str, children: Vec<ProjectFile>) -> Self {
        Self {
            path: path.to_string(),
            name: name.to_string(),
            file_type: FileType::Directory,
            is_directory: true,
            size_bytes: 0,
            git_status: GitStatus::Unchanged,
            children,
            is_expanded: true,
            depth: path.matches('/').count() as u32,
        }
    }
}

/// Generate mock file tree
pub fn mock_file_tree() -> Vec<ProjectFile> {
    vec![
        ProjectFile::directory("/workspace", "workspace", vec![
            ProjectFile::directory("/workspace/src", "src", vec![
                ProjectFile::file("/workspace/src/App.tsx", "App.tsx", FileType::TypeScript, 2840, GitStatus::Modified),
                ProjectFile::file("/workspace/src/index.tsx", "index.tsx", FileType::TypeScript, 420, GitStatus::Unchanged),
                ProjectFile::directory("/workspace/src/components", "components", vec![
                    ProjectFile::file("/workspace/src/components/Header.tsx", "Header.tsx", FileType::TypeScript, 1560, GitStatus::Modified),
                    ProjectFile::file("/workspace/src/components/Footer.tsx", "Footer.tsx", FileType::TypeScript, 890, GitStatus::Unchanged),
                    ProjectFile::file("/workspace/src/components/Sidebar.tsx", "Sidebar.tsx", FileType::TypeScript, 2100, GitStatus::Added),
                    ProjectFile::file("/workspace/src/components/Button.tsx", "Button.tsx", FileType::TypeScript, 780, GitStatus::Unchanged),
                    ProjectFile::file("/workspace/src/components/Card.tsx", "Card.tsx", FileType::TypeScript, 920, GitStatus::Unchanged),
                ]),
                ProjectFile::directory("/workspace/src/pages", "pages", vec![
                    ProjectFile::file("/workspace/src/pages/Home.tsx", "Home.tsx", FileType::TypeScript, 3200, GitStatus::Unchanged),
                    ProjectFile::file("/workspace/src/pages/Dashboard.tsx", "Dashboard.tsx", FileType::TypeScript, 4500, GitStatus::Modified),
                    ProjectFile::file("/workspace/src/pages/Settings.tsx", "Settings.tsx", FileType::TypeScript, 2800, GitStatus::Unchanged),
                    ProjectFile::file("/workspace/src/pages/Profile.tsx", "Profile.tsx", FileType::TypeScript, 1900, GitStatus::Untracked),
                ]),
                ProjectFile::directory("/workspace/src/hooks", "hooks", vec![
                    ProjectFile::file("/workspace/src/hooks/useAuth.ts", "useAuth.ts", FileType::TypeScript, 1200, GitStatus::Unchanged),
                    ProjectFile::file("/workspace/src/hooks/useApi.ts", "useApi.ts", FileType::TypeScript, 890, GitStatus::Unchanged),
                ]),
                ProjectFile::directory("/workspace/src/styles", "styles", vec![
                    ProjectFile::file("/workspace/src/styles/globals.css", "globals.css", FileType::Css, 2400, GitStatus::Modified),
                    ProjectFile::file("/workspace/src/styles/components.css", "components.css", FileType::Css, 1800, GitStatus::Unchanged),
                ]),
            ]),
            ProjectFile::directory("/workspace/public", "public", vec![
                ProjectFile::file("/workspace/public/favicon.ico", "favicon.ico", FileType::Image, 4200, GitStatus::Unchanged),
                ProjectFile::file("/workspace/public/logo.svg", "logo.svg", FileType::Image, 1200, GitStatus::Unchanged),
            ]),
            ProjectFile::file("/workspace/package.json", "package.json", FileType::Json, 1450, GitStatus::Modified),
            ProjectFile::file("/workspace/tsconfig.json", "tsconfig.json", FileType::Json, 620, GitStatus::Unchanged),
            ProjectFile::file("/workspace/tailwind.config.js", "tailwind.config.js", FileType::JavaScript, 380, GitStatus::Unchanged),
            ProjectFile::file("/workspace/README.md", "README.md", FileType::Markdown, 2100, GitStatus::Unchanged),
        ]),
        ProjectFile::directory("/logs", "logs", vec![
            ProjectFile::file("/logs/build.log", "build.log", FileType::Unknown, 45000, GitStatus::Unchanged),
            ProjectFile::file("/logs/agent.atif", "agent.atif", FileType::Unknown, 12000, GitStatus::Unchanged),
        ]),
        ProjectFile::directory("/db", "db", vec![
            ProjectFile::file("/db/data.sqlite", "data.sqlite", FileType::Unknown, 524288, GitStatus::Unchanged),
        ]),
    ]
}

// ============================================================================
// Editor Types
// ============================================================================

/// An open file tab in the editor
#[derive(Clone, Debug)]
pub struct EditorTab {
    pub path: String,
    pub name: String,
    pub file_type: FileType,
    pub is_modified: bool,
    pub is_active: bool,
    pub cursor_line: u32,
    pub cursor_column: u32,
}

impl EditorTab {
    pub fn mock(path: &str, name: &str, file_type: FileType, modified: bool, active: bool) -> Self {
        Self {
            path: path.to_string(),
            name: name.to_string(),
            file_type,
            is_modified: modified,
            is_active: active,
            cursor_line: 1,
            cursor_column: 1,
        }
    }
}

/// Generate mock editor tabs
pub fn mock_editor_tabs() -> Vec<EditorTab> {
    vec![
        EditorTab::mock("/workspace/src/App.tsx", "App.tsx", FileType::TypeScript, true, true),
        EditorTab::mock("/workspace/src/components/Header.tsx", "Header.tsx", FileType::TypeScript, true, false),
        EditorTab::mock("/workspace/src/pages/Dashboard.tsx", "Dashboard.tsx", FileType::TypeScript, false, false),
        EditorTab::mock("/workspace/src/styles/globals.css", "globals.css", FileType::Css, true, false),
    ]
}

/// Mock file content for the editor
pub fn mock_file_content() -> &'static str {
    r#"import React, { useState, useEffect } from 'react';
import { Header } from './components/Header';
import { Sidebar } from './components/Sidebar';
import { Dashboard } from './pages/Dashboard';
import { useAuth } from './hooks/useAuth';
import './styles/globals.css';

export default function App() {
  const { user, isLoading } = useAuth();
  const [sidebarOpen, setSidebarOpen] = useState(true);

  if (isLoading) {
    return <LoadingSpinner />;
  }

  if (!user) {
    return <LoginPage />;
  }

  return (
    <div className="app-container">
      <Header
        user={user}
        onMenuClick={() => setSidebarOpen(!sidebarOpen)}
      />
      <div className="main-layout">
        <Sidebar
          isOpen={sidebarOpen}
          currentPath={window.location.pathname}
        />
        <main className="content">
          <Dashboard userId={user.id} />
        </main>
      </div>
    </div>
  );
}

function LoadingSpinner() {
  return (
    <div className="loading-container">
      <div className="spinner" />
      <span>Loading...</span>
    </div>
  );
}

function LoginPage() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    // Auth logic here
  };

  return (
    <form onSubmit={handleSubmit} className="login-form">
      <h1>Welcome Back</h1>
      <input
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="Email"
      />
      <input
        type="password"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        placeholder="Password"
      />
      <button type="submit">Sign In</button>
    </form>
  );
}"#
}

// ============================================================================
// Agent Types
// ============================================================================

/// Agent mode
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentMode {
    #[default]
    Agent,
    Chat,
    Off,
}

impl AgentMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Agent => "AGENT",
            Self::Chat => "CHAT",
            Self::Off => "OFF",
        }
    }
}

/// Status of an agent task
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentTaskStatus {
    #[default]
    Pending,
    Running,
    Completed,
    Failed,
}

impl AgentTaskStatus {
    pub fn indicator(&self) -> &'static str {
        match self {
            Self::Pending => "○",
            Self::Running => "◐",
            Self::Completed => "●",
            Self::Failed => "✕",
        }
    }
}

/// An agent task in the task feed
#[derive(Clone, Debug)]
pub struct AgentTask {
    pub id: String,
    pub description: String,
    pub status: AgentTaskStatus,
    pub files_changed: Vec<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub tokens_used: u32,
}

impl AgentTask {
    pub fn mock(id: &str, desc: &str, status: AgentTaskStatus) -> Self {
        Self {
            id: id.to_string(),
            description: desc.to_string(),
            status,
            files_changed: vec!["src/App.tsx".to_string(), "src/components/Header.tsx".to_string()],
            started_at: "10:45:23".to_string(),
            completed_at: if status == AgentTaskStatus::Completed { Some("10:47:12".to_string()) } else { None },
            tokens_used: 2450,
        }
    }
}

/// Generate mock agent tasks
pub fn mock_agent_tasks() -> Vec<AgentTask> {
    vec![
        AgentTask::mock("task_1", "Add user authentication with email/password", AgentTaskStatus::Completed),
        AgentTask::mock("task_2", "Create responsive navigation header", AgentTaskStatus::Completed),
        AgentTask::mock("task_3", "Implement dashboard with charts", AgentTaskStatus::Running),
        AgentTask::mock("task_4", "Add dark mode support", AgentTaskStatus::Pending),
        AgentTask::mock("task_5", "Set up database migrations", AgentTaskStatus::Pending),
    ]
}

// ============================================================================
// Database Types
// ============================================================================

/// Column data type
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColumnType {
    #[default]
    Text,
    Integer,
    Real,
    Boolean,
    Timestamp,
    Uuid,
    Json,
    Blob,
}

impl ColumnType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Text => "TEXT",
            Self::Integer => "INTEGER",
            Self::Real => "REAL",
            Self::Boolean => "BOOLEAN",
            Self::Timestamp => "TIMESTAMP",
            Self::Uuid => "UUID",
            Self::Json => "JSON",
            Self::Blob => "BLOB",
        }
    }
}

/// A database column definition
#[derive(Clone, Debug)]
pub struct DatabaseColumn {
    pub name: String,
    pub column_type: ColumnType,
    pub is_primary_key: bool,
    pub is_nullable: bool,
    pub default_value: Option<String>,
    pub is_unique: bool,
    pub foreign_key: Option<String>,
}

impl DatabaseColumn {
    pub fn primary(name: &str, col_type: ColumnType) -> Self {
        Self {
            name: name.to_string(),
            column_type: col_type,
            is_primary_key: true,
            is_nullable: false,
            default_value: None,
            is_unique: true,
            foreign_key: None,
        }
    }

    pub fn required(name: &str, col_type: ColumnType) -> Self {
        Self {
            name: name.to_string(),
            column_type: col_type,
            is_primary_key: false,
            is_nullable: false,
            default_value: None,
            is_unique: false,
            foreign_key: None,
        }
    }

    pub fn nullable(name: &str, col_type: ColumnType) -> Self {
        Self {
            name: name.to_string(),
            column_type: col_type,
            is_primary_key: false,
            is_nullable: true,
            default_value: None,
            is_unique: false,
            foreign_key: None,
        }
    }

    pub fn with_default(mut self, default: &str) -> Self {
        self.default_value = Some(default.to_string());
        self
    }

    pub fn with_foreign_key(mut self, fk: &str) -> Self {
        self.foreign_key = Some(fk.to_string());
        self
    }
}

/// A database table
#[derive(Clone, Debug)]
pub struct DatabaseTable {
    pub name: String,
    pub columns: Vec<DatabaseColumn>,
    pub row_count: u64,
    pub size_bytes: u64,
    pub indexes: Vec<String>,
}

impl DatabaseTable {
    pub fn mock(name: &str, columns: Vec<DatabaseColumn>, rows: u64) -> Self {
        Self {
            name: name.to_string(),
            columns,
            row_count: rows,
            size_bytes: rows * 256, // rough estimate
            indexes: vec![format!("{}_pkey", name)],
        }
    }
}

/// A row of data (column name -> value)
pub type DatabaseRow = HashMap<String, String>;

/// Generate mock database tables
pub fn mock_database_tables() -> Vec<DatabaseTable> {
    vec![
        DatabaseTable::mock(
            "users",
            vec![
                DatabaseColumn::primary("id", ColumnType::Uuid),
                DatabaseColumn::required("email", ColumnType::Text),
                DatabaseColumn::required("password_hash", ColumnType::Text),
                DatabaseColumn::nullable("name", ColumnType::Text),
                DatabaseColumn::nullable("avatar_url", ColumnType::Text),
                DatabaseColumn::required("created_at", ColumnType::Timestamp).with_default("NOW()"),
                DatabaseColumn::nullable("last_login", ColumnType::Timestamp),
                DatabaseColumn::required("is_active", ColumnType::Boolean).with_default("true"),
            ],
            1_247,
        ),
        DatabaseTable::mock(
            "products",
            vec![
                DatabaseColumn::primary("id", ColumnType::Uuid),
                DatabaseColumn::required("name", ColumnType::Text),
                DatabaseColumn::nullable("description", ColumnType::Text),
                DatabaseColumn::required("price_cents", ColumnType::Integer),
                DatabaseColumn::required("stock", ColumnType::Integer).with_default("0"),
                DatabaseColumn::nullable("category", ColumnType::Text),
                DatabaseColumn::required("created_at", ColumnType::Timestamp).with_default("NOW()"),
                DatabaseColumn::required("is_active", ColumnType::Boolean).with_default("true"),
            ],
            3_891,
        ),
        DatabaseTable::mock(
            "orders",
            vec![
                DatabaseColumn::primary("id", ColumnType::Uuid),
                DatabaseColumn::required("user_id", ColumnType::Uuid).with_foreign_key("users.id"),
                DatabaseColumn::required("status", ColumnType::Text).with_default("'pending'"),
                DatabaseColumn::required("total_cents", ColumnType::Integer),
                DatabaseColumn::required("created_at", ColumnType::Timestamp).with_default("NOW()"),
                DatabaseColumn::nullable("completed_at", ColumnType::Timestamp),
                DatabaseColumn::nullable("shipping_address", ColumnType::Json),
            ],
            8_456,
        ),
        DatabaseTable::mock(
            "order_items",
            vec![
                DatabaseColumn::primary("id", ColumnType::Uuid),
                DatabaseColumn::required("order_id", ColumnType::Uuid).with_foreign_key("orders.id"),
                DatabaseColumn::required("product_id", ColumnType::Uuid).with_foreign_key("products.id"),
                DatabaseColumn::required("quantity", ColumnType::Integer),
                DatabaseColumn::required("price_cents", ColumnType::Integer),
            ],
            24_102,
        ),
        DatabaseTable::mock(
            "sessions",
            vec![
                DatabaseColumn::primary("id", ColumnType::Uuid),
                DatabaseColumn::required("user_id", ColumnType::Uuid).with_foreign_key("users.id"),
                DatabaseColumn::required("token", ColumnType::Text),
                DatabaseColumn::required("expires_at", ColumnType::Timestamp),
                DatabaseColumn::required("created_at", ColumnType::Timestamp).with_default("NOW()"),
            ],
            892,
        ),
    ]
}

/// Generate mock rows for a table
pub fn mock_table_rows(table_name: &str) -> Vec<DatabaseRow> {
    match table_name {
        "users" => vec![
            [
                ("id".to_string(), "a1b2c3d4-...".to_string()),
                ("email".to_string(), "alice@example.com".to_string()),
                ("name".to_string(), "Alice Johnson".to_string()),
                ("created_at".to_string(), "2024-11-15 09:30:00".to_string()),
                ("is_active".to_string(), "true".to_string()),
            ].into_iter().collect(),
            [
                ("id".to_string(), "e5f6g7h8-...".to_string()),
                ("email".to_string(), "bob@example.com".to_string()),
                ("name".to_string(), "Bob Smith".to_string()),
                ("created_at".to_string(), "2024-11-18 14:22:00".to_string()),
                ("is_active".to_string(), "true".to_string()),
            ].into_iter().collect(),
            [
                ("id".to_string(), "i9j0k1l2-...".to_string()),
                ("email".to_string(), "carol@example.com".to_string()),
                ("name".to_string(), "Carol Williams".to_string()),
                ("created_at".to_string(), "2024-12-01 11:45:00".to_string()),
                ("is_active".to_string(), "false".to_string()),
            ].into_iter().collect(),
        ],
        "products" => vec![
            [
                ("id".to_string(), "p1r2o3d4-...".to_string()),
                ("name".to_string(), "Wireless Headphones".to_string()),
                ("price_cents".to_string(), "7999".to_string()),
                ("stock".to_string(), "142".to_string()),
                ("category".to_string(), "Electronics".to_string()),
            ].into_iter().collect(),
            [
                ("id".to_string(), "p5r6o7d8-...".to_string()),
                ("name".to_string(), "Running Shoes".to_string()),
                ("price_cents".to_string(), "12999".to_string()),
                ("stock".to_string(), "58".to_string()),
                ("category".to_string(), "Apparel".to_string()),
            ].into_iter().collect(),
        ],
        _ => vec![],
    }
}

// ============================================================================
// Deployment Types
// ============================================================================

/// Deployment status
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeploymentStatus {
    #[default]
    Pending,
    Building,
    Deploying,
    Live,
    Failed,
    Rolled,
}

impl DeploymentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Building => "BUILDING",
            Self::Deploying => "DEPLOYING",
            Self::Live => "LIVE",
            Self::Failed => "FAILED",
            Self::Rolled => "ROLLED BACK",
        }
    }

    pub fn indicator(&self) -> &'static str {
        match self {
            Self::Pending => "○",
            Self::Building => "◐",
            Self::Deploying => "◐",
            Self::Live => "●",
            Self::Failed => "✕",
            Self::Rolled => "↺",
        }
    }
}

/// A deployment record
#[derive(Clone, Debug)]
pub struct Deployment {
    pub id: String,
    pub version: String,
    pub commit_hash: String,
    pub commit_message: String,
    pub status: DeploymentStatus,
    pub created_at: String,
    pub deployed_at: Option<String>,
    pub url: String,
    pub build_duration_secs: Option<u32>,
}

impl Deployment {
    pub fn mock(version: &str, status: DeploymentStatus) -> Self {
        Self {
            id: format!("deploy_{}", version.replace('.', "_")),
            version: version.to_string(),
            commit_hash: "abc123f".to_string(),
            commit_message: "Update dashboard components".to_string(),
            status,
            created_at: "2024-12-10 14:30:00".to_string(),
            deployed_at: if status == DeploymentStatus::Live { Some("2024-12-10 14:32:15".to_string()) } else { None },
            url: format!("https://v{}.preview.vibe.dev", version.replace('.', "-")),
            build_duration_secs: Some(135),
        }
    }
}

/// Generate mock deployments
pub fn mock_deployments() -> Vec<Deployment> {
    vec![
        {
            let mut d = Deployment::mock("1.4.2", DeploymentStatus::Live);
            d.commit_message = "Add user settings page".to_string();
            d
        },
        {
            let mut d = Deployment::mock("1.4.1", DeploymentStatus::Rolled);
            d.commit_message = "Fix auth redirect bug".to_string();
            d.created_at = "2024-12-09 11:20:00".to_string();
            d
        },
        {
            let mut d = Deployment::mock("1.4.0", DeploymentStatus::Live);
            d.commit_message = "Major dashboard redesign".to_string();
            d.created_at = "2024-12-08 16:45:00".to_string();
            d
        },
        {
            let mut d = Deployment::mock("1.3.5", DeploymentStatus::Failed);
            d.commit_message = "Experimental feature branch".to_string();
            d.created_at = "2024-12-07 09:30:00".to_string();
            d
        },
    ]
}

/// Domain status
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DomainStatus {
    #[default]
    Pending,
    Verifying,
    Active,
    Error,
}

impl DomainStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Verifying => "VERIFYING",
            Self::Active => "ACTIVE",
            Self::Error => "ERROR",
        }
    }
}

/// A custom domain
#[derive(Clone, Debug)]
pub struct Domain {
    pub domain: String,
    pub status: DomainStatus,
    pub ssl_expires: Option<String>,
    pub is_primary: bool,
}

impl Domain {
    pub fn mock(domain: &str, status: DomainStatus, primary: bool) -> Self {
        Self {
            domain: domain.to_string(),
            status,
            ssl_expires: if status == DomainStatus::Active { Some("2025-12-10".to_string()) } else { None },
            is_primary: primary,
        }
    }
}

/// Generate mock domains
pub fn mock_domains() -> Vec<Domain> {
    vec![
        Domain::mock("myapp.vibe.dev", DomainStatus::Active, false),
        Domain::mock("app.mycompany.com", DomainStatus::Active, true),
        Domain::mock("staging.mycompany.com", DomainStatus::Verifying, false),
    ]
}

// ============================================================================
// Analytics Types
// ============================================================================

/// Time range for analytics
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnalyticsRange {
    Today,
    #[default]
    Week,
    Month,
    Year,
}

impl AnalyticsRange {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Today => "TODAY",
            Self::Week => "7 DAYS",
            Self::Month => "30 DAYS",
            Self::Year => "YEAR",
        }
    }

    pub fn all() -> &'static [AnalyticsRange] {
        &[Self::Today, Self::Week, Self::Month, Self::Year]
    }
}

/// Analytics data point
#[derive(Clone, Debug)]
pub struct AnalyticsDataPoint {
    pub label: String,
    pub value: u64,
}

/// Analytics summary
#[derive(Clone, Debug, Default)]
pub struct AnalyticsSummary {
    pub page_views: u64,
    pub unique_visitors: u64,
    pub avg_session_duration_secs: u32,
    pub bounce_rate: f32,
    pub top_pages: Vec<(String, u64)>,
    pub traffic_by_day: Vec<AnalyticsDataPoint>,
    pub traffic_by_country: Vec<(String, u64)>,
    pub traffic_by_device: Vec<(String, u64)>,
}

/// Generate mock analytics
pub fn mock_analytics() -> AnalyticsSummary {
    AnalyticsSummary {
        page_views: 45_892,
        unique_visitors: 12_456,
        avg_session_duration_secs: 245,
        bounce_rate: 42.3,
        top_pages: vec![
            ("/".to_string(), 18_456),
            ("/dashboard".to_string(), 12_890),
            ("/settings".to_string(), 5_432),
            ("/profile".to_string(), 4_210),
            ("/products".to_string(), 3_904),
        ],
        traffic_by_day: vec![
            AnalyticsDataPoint { label: "Mon".to_string(), value: 5_420 },
            AnalyticsDataPoint { label: "Tue".to_string(), value: 6_120 },
            AnalyticsDataPoint { label: "Wed".to_string(), value: 7_890 },
            AnalyticsDataPoint { label: "Thu".to_string(), value: 8_450 },
            AnalyticsDataPoint { label: "Fri".to_string(), value: 7_200 },
            AnalyticsDataPoint { label: "Sat".to_string(), value: 5_102 },
            AnalyticsDataPoint { label: "Sun".to_string(), value: 5_710 },
        ],
        traffic_by_country: vec![
            ("US".to_string(), 8_920),
            ("UK".to_string(), 1_245),
            ("DE".to_string(), 892),
            ("CA".to_string(), 756),
            ("Other".to_string(), 643),
        ],
        traffic_by_device: vec![
            ("Desktop".to_string(), 7_890),
            ("Mobile".to_string(), 3_456),
            ("Tablet".to_string(), 1_110),
        ],
    }
}

// ============================================================================
// Terminal Types
// ============================================================================

/// Terminal line type
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerminalLineType {
    #[default]
    Output,
    Input,
    Error,
    System,
}

/// A line in the terminal
#[derive(Clone, Debug)]
pub struct TerminalLine {
    pub content: String,
    pub line_type: TerminalLineType,
    pub timestamp: String,
}

impl TerminalLine {
    pub fn output(content: &str) -> Self {
        Self {
            content: content.to_string(),
            line_type: TerminalLineType::Output,
            timestamp: "10:45:23".to_string(),
        }
    }

    pub fn input(content: &str) -> Self {
        Self {
            content: content.to_string(),
            line_type: TerminalLineType::Input,
            timestamp: "10:45:22".to_string(),
        }
    }

    pub fn error(content: &str) -> Self {
        Self {
            content: content.to_string(),
            line_type: TerminalLineType::Error,
            timestamp: "10:45:24".to_string(),
        }
    }

    pub fn system(content: &str) -> Self {
        Self {
            content: content.to_string(),
            line_type: TerminalLineType::System,
            timestamp: "10:45:20".to_string(),
        }
    }
}

/// Generate mock terminal output
pub fn mock_terminal_lines() -> Vec<TerminalLine> {
    vec![
        TerminalLine::system("OANIX Shell v0.1.0"),
        TerminalLine::system("Namespace: /workspace mounted"),
        TerminalLine::input("$ bun run dev"),
        TerminalLine::output("$ bun run v1.1.0"),
        TerminalLine::output("Starting development server..."),
        TerminalLine::output(""),
        TerminalLine::output("  VITE v5.0.0  ready in 234 ms"),
        TerminalLine::output(""),
        TerminalLine::output("  > Local:   http://localhost:5173/"),
        TerminalLine::output("  > Network: http://192.168.1.5:5173/"),
        TerminalLine::output(""),
        TerminalLine::output("  press h to show help"),
        TerminalLine::output(""),
        TerminalLine::output("[HMR] Connected"),
        TerminalLine::output("[vite] page reload src/App.tsx"),
    ]
}

// ============================================================================
// Tab-specific State Types
// ============================================================================

/// State for the Projects tab
#[derive(Clone, Debug, Default)]
pub struct ProjectsTabState {
    pub projects: Vec<Project>,
    pub templates: Vec<ProjectTemplate>,
    pub search_query: String,
    pub selected_category: TemplateCategory,
    pub show_templates: bool,
    pub selected_project_id: Option<String>,
}

/// State for the Editor tab
#[derive(Clone, Debug, Default)]
pub struct EditorTabState {
    pub file_tree: Vec<ProjectFile>,
    pub open_tabs: Vec<EditorTab>,
    pub active_file_path: Option<String>,
    pub file_content: String,
    pub agent_mode: AgentMode,
    pub agent_tasks: Vec<AgentTask>,
    pub terminal_lines: Vec<TerminalLine>,
    pub terminal_input: String,
    pub show_preview: bool,
    pub show_terminal: bool,
    pub show_agent_panel: bool,
}

/// State for the Database tab
#[derive(Clone, Debug, Default)]
pub struct DatabaseTabState {
    pub tables: Vec<DatabaseTable>,
    pub selected_table: Option<String>,
    pub table_rows: Vec<DatabaseRow>,
    pub sql_query: String,
    pub query_results: Vec<DatabaseRow>,
    pub show_schema: bool,
}

/// State for the Deploy tab
#[derive(Clone, Debug, Default)]
pub struct DeployTabState {
    pub deployments: Vec<Deployment>,
    pub domains: Vec<Domain>,
    pub analytics: AnalyticsSummary,
    pub analytics_range: AnalyticsRange,
    pub show_analytics: bool,
}
