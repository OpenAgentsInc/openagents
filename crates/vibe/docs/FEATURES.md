# Vibe Features

Complete feature specification for Vibe—the AI-native development platform.

---

## Table of Contents

1. [AI Agent System](#ai-agent-system)
2. [Editor & IDE](#editor--ide)
3. [Design Mode](#design-mode)
4. [Backend & Database](#backend--database)
5. [Deployment & Hosting](#deployment--hosting)
6. [Collaboration](#collaboration)
7. [Integrations](#integrations)
8. [Templates & Scaffolding](#templates--scaffolding)
9. [Developer Experience](#developer-experience)
10. [Security & Compliance](#security--compliance)
11. [Analytics & Insights](#analytics--insights)
12. [Mobile & Responsive](#mobile--responsive)
13. [OpenAgents Ecosystem](#openagents-ecosystem)

---

## AI Agent System

### Agent Modes

#### Agent Mode (Default)
The AI operates autonomously, executing multi-step tasks without constant prompting.

**Capabilities:**
- Interprets natural language requests
- Breaks complex tasks into subtasks
- Creates, modifies, and deletes files
- Fixes bugs automatically when detected
- Searches documentation when implementing unfamiliar APIs
- Generates images and assets
- Self-verifies by running the app and checking output
- Iterates until the task is complete

**Example:**
```
User: "Add user authentication with email and Google OAuth"

Agent executes:
1. Analyze current project structure
2. Create user table in database
3. Set up auth providers
4. Create login/signup pages
5. Add protected route middleware
6. Update navigation with user state
7. Test login flow
8. Report completion
```

#### Chat Mode
Conversational mode for brainstorming and planning without making changes.

**Use cases:**
- Discuss architecture decisions
- Debug issues collaboratively
- Get implementation recommendations
- Plan feature rollout
- Review code explanations

**Example:**
```
User: "How should I structure the payment flow?"

Agent: "I recommend:
1. Cart review page with order summary
2. Shipping address form (if physical)
3. Payment method selection
4. Confirmation page with receipt

Want me to implement this plan?"

User: "Yes, implement it"

[Switches to Agent Mode and executes]
```

### Task Feed

Real-time visibility into agent work:

```
┌─────────────────────────────────────────┐
│ Building your app...                    │
├─────────────────────────────────────────┤
│ ✓ Analyzing project structure           │
│ ✓ Creating database schema              │
│ ● Building user interface...            │
│ ○ Setting up authentication             │
│ ○ Testing login flow                    │
└─────────────────────────────────────────┘
```

**Features:**
- Collapsible task list (collapsed by default)
- Expand to see detailed actions
- Each task shows status (pending, running, complete, error)
- Click task to see files changed
- Cancel in-progress work

### Code Attribution

Every change the agent makes is tracked:

```typescript
// Agent modified: src/components/Button.tsx:42
// Changed: background-color from blue-500 to indigo-600
```

**Features:**
- Shows exact file and line number
- Displays before/after diff
- Links to code in editor
- Builds trust through transparency

### Self-Healing

When errors occur, the agent attempts automatic recovery:

1. Detects runtime errors in preview
2. Analyzes error message and stack trace
3. Identifies problematic code
4. Generates fix
5. Re-runs and verifies

**Example:**
```
Agent: "I detected a TypeError in the checkout component.
The issue was accessing a property on undefined.
I've added a null check. The error is now resolved."
```

### Agent Tools

Built-in tools the agent can use:

| Tool | Description |
|------|-------------|
| `file_read` | Read file contents |
| `file_write` | Create or update files |
| `file_delete` | Remove files |
| `shell_exec` | Run terminal commands |
| `web_search` | Search documentation |
| `image_generate` | Create AI images |
| `database_query` | Query project database |
| `api_call` | Make HTTP requests |
| `git_commit` | Version control operations |

---

## Editor & IDE

### Code Editor

Full-featured code editor built on GPUI:

**Core Features:**
- Syntax highlighting for 50+ languages
- Multi-cursor editing
- Code folding
- Bracket matching
- Auto-indentation
- Line numbers with click-to-select
- Minimap navigation
- Split panes (horizontal/vertical)

**Intelligence:**
- IntelliSense autocomplete
- Type inference and hints
- Error/warning squiggles
- Quick fixes and suggestions
- Go to definition
- Find all references
- Rename symbol
- Parameter hints

**Search:**
- File search (fuzzy matching)
- Project-wide text search
- Search and replace
- Regular expression support
- Search in selection

### File Tree

```
┌─ my-app
├── src/
│   ├── components/
│   │   ├── Button.tsx
│   │   └── Header.tsx
│   ├── pages/
│   │   ├── Home.tsx
│   │   └── Dashboard.tsx
│   └── App.tsx
├── public/
│   └── favicon.ico
├── package.json
└── vibe.toml
```

**Features:**
- Drag-and-drop file organization
- Right-click context menu
- New file/folder creation
- Rename, delete, duplicate
- Search within tree
- Collapse/expand all
- File type icons
- Git status indicators

### Tab Management

**Features:**
- Unlimited open tabs
- Tab reordering via drag
- Close tab (X button)
- Close others / Close all
- Pin important tabs
- Tab preview on hover
- Recently closed (reopen)
- Split to new pane

### Terminal

Integrated terminal connected to OANIX:

**Features:**
- Multiple terminal instances
- Named terminals
- Copy/paste support
- Clear terminal
- Kill process
- Search terminal output
- Link detection (clickable URLs)
- Color support (ANSI)

### ATIF Trajectory Viewer

Browse agent action history:

```
┌─────────────────────────────────────────┐
│ Session: vibe-session-abc123            │
│ Agent: vibe-builder v1.0                │
│ Steps: 24                               │
├─────────────────────────────────────────┤
│ Step 1: Read package.json               │
│ Step 2: Write src/App.tsx               │
│ Step 3: Run npm install                 │
│ Step 4: Fix TypeScript error            │
│ ...                                     │
└─────────────────────────────────────────┘
```

**Features:**
- Timeline view of all actions
- Filter by action type
- Search within trajectory
- Export trajectory JSON
- Share trajectory link
- Compare trajectories

---

## Design Mode

### Visual Editor

Edit UI without touching code:

**Selection:**
- Click any element to select
- Blue outline shows selection
- Handles for resizing
- Parent/child navigation

**Properties Panel:**
```
┌─────────────────────────────────────────┐
│ Button                                  │
├─────────────────────────────────────────┤
│ Size                                    │
│ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐        │
│ │ sm  │ │ md  │ │ lg  │ │ xl  │        │
│ └─────┘ └─────┘ └─────┘ └─────┘        │
├─────────────────────────────────────────┤
│ Variant                                 │
│ ● Primary  ○ Secondary  ○ Ghost        │
├─────────────────────────────────────────┤
│ Text: "Get Started"                     │
├─────────────────────────────────────────┤
│ Spacing                                 │
│ M: 16  8  16  8    P: 12  24  12  24   │
└─────────────────────────────────────────┘
```

### Theme System

Global styling across entire project:

**Theme Editor:**
```
┌─────────────────────────────────────────┐
│ Theme: Dark Mode                        │
├─────────────────────────────────────────┤
│ Colors                                  │
│ Primary:    [■] #6366f1                 │
│ Secondary:  [■] #8b5cf6                 │
│ Background: [■] #0f172a                 │
│ Surface:    [■] #1e293b                 │
│ Text:       [■] #f8fafc                 │
├─────────────────────────────────────────┤
│ Typography                              │
│ Font:       Inter                       │
│ Base Size:  16px                        │
│ Scale:      1.25                        │
├─────────────────────────────────────────┤
│ Spacing                                 │
│ Base:       4px                         │
│ Scale:      2, 4, 8, 16, 24, 32, 48    │
├─────────────────────────────────────────┤
│ Border Radius                           │
│ Default:    8px                         │
│ Button:     6px                         │
│ Card:       12px                        │
└─────────────────────────────────────────┘
```

**Features:**
- Save custom themes
- Import themes from URL
- Export theme config
- Dark/light mode variants
- Preview theme changes live

### Layout Controls

**Spacing:**
- Visual margin/padding controls
- Click and drag to adjust
- Numeric input for precision
- Presets (0, 1, 2, 4, 8, 16, etc.)

**Flexbox/Grid:**
- Direction toggle (row/column)
- Justify content selector
- Align items selector
- Gap control
- Wrap toggle

**Position:**
- Static, relative, absolute, fixed
- Top/right/bottom/left
- Z-index control

### Typography Panel

```
┌─────────────────────────────────────────┐
│ Typography                              │
├─────────────────────────────────────────┤
│ Font Family                             │
│ [Inter                            ▼]   │
├─────────────────────────────────────────┤
│ Size        Weight      Line Height    │
│ [16px ▼]    [Medium ▼]  [1.5   ▼]     │
├─────────────────────────────────────────┤
│ Alignment                               │
│ [≡] [≡] [≡] [≡]                        │
│ Left Center Right Justify              │
├─────────────────────────────────────────┤
│ Transform                               │
│ [Aa] [AA] [aa]                         │
│ Normal Upper Lower                      │
└─────────────────────────────────────────┘
```

### Color Controls

**Color Picker:**
- Hex, RGB, HSL input
- Eyedropper tool
- Recent colors
- Theme color swatches
- Opacity slider
- Gradient support

**Applies to:**
- Text color
- Background color
- Border color
- Shadow color

### Image Management

**Upload:**
- Drag and drop
- Click to browse
- Paste from clipboard
- URL import

**AI Generation:**
- Describe desired image
- Style options (photo, illustration, icon)
- Aspect ratio selection
- Generate variations

**Optimization:**
- Auto-compress on upload
- WebP conversion
- Responsive srcset
- Lazy loading

### Responsive Preview

```
┌─────────────────────────────────────────┐
│ [Desktop] [Tablet] [Mobile] [Custom]   │
├─────────────────────────────────────────┤
│                                         │
│    ┌─────────────────────────────┐     │
│    │                             │     │
│    │        Preview Area         │     │
│    │                             │     │
│    │                             │     │
│    └─────────────────────────────┘     │
│                                         │
│    1024 × 768                          │
└─────────────────────────────────────────┘
```

**Features:**
- Preset device sizes
- Custom dimensions
- Portrait/landscape toggle
- Zoom in/out
- Device frame overlay

---

## Backend & Database

### Database Dashboard

```
┌─────────────────────────────────────────┐
│ Database: my-app-db                     │
├─────────────────────────────────────────┤
│ Tables                                  │
│ ├── users (1,234 rows)                  │
│ ├── products (89 rows)                  │
│ ├── orders (456 rows)                   │
│ └── order_items (1,823 rows)            │
├─────────────────────────────────────────┤
│ [+ New Table] [SQL Editor] [Migrations] │
└─────────────────────────────────────────┘
```

**Table Browser:**
- View all rows with pagination
- Filter by column values
- Sort by any column
- Edit cells inline
- Add/delete rows
- Export to CSV

**SQL Editor:**
- Write and run SQL queries
- Syntax highlighting
- Auto-complete table/column names
- Query history
- Save favorite queries
- Export results

### Schema Management

**Visual Schema Editor:**
```
┌─────────────────┐      ┌─────────────────┐
│     users       │      │     orders      │
├─────────────────┤      ├─────────────────┤
│ id (PK)         │──┐   │ id (PK)         │
│ email           │  │   │ user_id (FK) ───│──┐
│ name            │  │   │ total           │  │
│ created_at      │  └──▶│ status          │  │
└─────────────────┘      │ created_at      │  │
                         └─────────────────┘  │
                                              │
┌─────────────────┐                           │
│  order_items    │                           │
├─────────────────┤                           │
│ id (PK)         │                           │
│ order_id (FK) ──│───────────────────────────┘
│ product_id (FK) │
│ quantity        │
│ price           │
└─────────────────┘
```

**Features:**
- Drag to create relationships
- Edit column types
- Add indexes
- Generate migrations
- Rollback migrations

### Authentication Dashboard

```
┌─────────────────────────────────────────┐
│ Authentication                          │
├─────────────────────────────────────────┤
│ Providers                               │
│ ✓ Email/Password                        │
│ ✓ Magic Link                            │
│ ✓ Google OAuth                          │
│ ○ GitHub OAuth                          │
│ ○ Apple Sign-In                         │
├─────────────────────────────────────────┤
│ Users: 1,234 total                      │
│ Active Sessions: 89                     │
│ Sign-ups Today: 12                      │
├─────────────────────────────────────────┤
│ [User List] [Sessions] [Settings]       │
└─────────────────────────────────────────┘
```

**User Management:**
- View all users
- Search by email/name
- Edit user details
- Reset password
- Ban/unban users
- View login history

**Session Management:**
- Active sessions
- Revoke sessions
- Session duration settings
- Remember me options

### File Storage

```
┌─────────────────────────────────────────┐
│ Storage                                 │
├─────────────────────────────────────────┤
│ Buckets                                 │
│ ├── avatars/ (245 files, 12 MB)         │
│ ├── uploads/ (1,234 files, 89 MB)       │
│ └── assets/ (34 files, 5 MB)            │
├─────────────────────────────────────────┤
│ Usage: 106 MB / 1 GB                    │
├─────────────────────────────────────────┤
│ [+ New Bucket] [Upload] [Settings]      │
└─────────────────────────────────────────┘
```

**Features:**
- Create buckets
- Upload files
- Folder organization
- Public/private access
- Direct URLs
- Image transformations
- Bandwidth tracking

### Edge Functions

Serverless functions for custom logic:

```typescript
// functions/send-email.ts
export async function handler(req: Request) {
  const { to, subject, body } = await req.json();

  await sendEmail({ to, subject, body });

  return Response.json({ success: true });
}
```

**Dashboard:**
- List all functions
- View logs
- Execution metrics
- Environment variables
- Deploy history
- Rollback support

### API Explorer

Test your API endpoints:

```
┌─────────────────────────────────────────┐
│ API Explorer                            │
├─────────────────────────────────────────┤
│ GET  /api/users                         │
│ GET  /api/users/:id                     │
│ POST /api/users                         │
│ PUT  /api/users/:id                     │
│ DELETE /api/users/:id                   │
├─────────────────────────────────────────┤
│ Request                                 │
│ ┌─────────────────────────────────────┐ │
│ │ GET /api/users?limit=10             │ │
│ └─────────────────────────────────────┘ │
│ [Send Request]                          │
├─────────────────────────────────────────┤
│ Response (200 OK, 45ms)                 │
│ ┌─────────────────────────────────────┐ │
│ │ [{ "id": 1, "name": "John" }, ...]  │ │
│ └─────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

---

## Deployment & Hosting

### One-Click Publish

```
┌─────────────────────────────────────────┐
│         [  Publish  ]                   │
├─────────────────────────────────────────┤
│ Preview: https://my-app-preview.vibe.dev│
│ Production: https://my-app.vibe.dev     │
└─────────────────────────────────────────┘
```

**What happens on publish:**
1. Build production bundle
2. Run security scan
3. Optimize assets
4. Deploy to edge network
5. Warm CDN cache
6. Update DNS
7. Notify collaborators

### Custom Domains

```
┌─────────────────────────────────────────┐
│ Domains                                 │
├─────────────────────────────────────────┤
│ my-app.vibe.dev (default)               │
│ www.myapp.com ✓ SSL active              │
│ myapp.com ✓ SSL active                  │
├─────────────────────────────────────────┤
│ [+ Add Domain]                          │
└─────────────────────────────────────────┘
```

**Features:**
- Automatic SSL provisioning
- Certificate renewal
- DNS verification
- Redirect configuration
- www/non-www handling

### Preview Deployments

Share work-in-progress:

- Unique URL per version
- Password protection option
- Comment on preview
- Compare to production
- Promote to production

### Deployment History

```
┌─────────────────────────────────────────┐
│ Deployments                             │
├─────────────────────────────────────────┤
│ ● v12 - Now (Production)                │
│   "Add checkout flow" - 2 hours ago     │
│                                         │
│ ○ v11                                   │
│   "Fix mobile menu" - 1 day ago         │
│                                         │
│ ○ v10                                   │
│   "Add product pages" - 2 days ago      │
├─────────────────────────────────────────┤
│ [Rollback] [Compare] [View]             │
└─────────────────────────────────────────┘
```

**Features:**
- Complete deploy history
- One-click rollback
- Side-by-side comparison
- Deploy logs
- Build artifacts

### Environment Variables

```
┌─────────────────────────────────────────┐
│ Environment Variables                   │
├─────────────────────────────────────────┤
│ STRIPE_SECRET_KEY     •••••••••••       │
│ DATABASE_URL          •••••••••••       │
│ SENDGRID_API_KEY      •••••••••••       │
├─────────────────────────────────────────┤
│ [+ Add Variable]                        │
└─────────────────────────────────────────┘
```

**Features:**
- Secure storage
- Per-environment values
- Bulk import
- Reference in code
- Audit log

---

## Collaboration

### Team Workspaces

```
┌─────────────────────────────────────────┐
│ Acme Corp Workspace                     │
├─────────────────────────────────────────┤
│ Projects                                │
│ ├── Marketing Site                      │
│ ├── Customer Portal                     │
│ └── Internal Tools                      │
├─────────────────────────────────────────┤
│ Members                                 │
│ ├── alice@acme.com (Admin)              │
│ ├── bob@acme.com (Editor)               │
│ └── carol@acme.com (Viewer)             │
├─────────────────────────────────────────┤
│ [+ New Project] [Invite] [Settings]     │
└─────────────────────────────────────────┘
```

### Role-Based Permissions

| Permission | Admin | Editor | Viewer |
|------------|-------|--------|--------|
| View projects | ✓ | ✓ | ✓ |
| Edit code | ✓ | ✓ | ✗ |
| Design mode | ✓ | ✓ | ✗ |
| Publish | ✓ | ✓ | ✗ |
| Manage members | ✓ | ✗ | ✗ |
| Billing | ✓ | ✗ | ✗ |
| Delete project | ✓ | ✗ | ✗ |

### Real-Time Collaboration

**Presence:**
- See who's online
- Cursors with names
- Typing indicators
- Currently editing file

**Sync:**
- Real-time code sync
- Design changes sync
- No conflicts
- Offline support with sync on reconnect

### Comments

```
┌─────────────────────────────────────────┐
│ Button.tsx:42                           │
├─────────────────────────────────────────┤
│ Alice: "Should we use primary color     │
│ here instead?"                          │
│                                         │
│ Bob: "Good catch, updating now."        │
│                                         │
│ [Reply] [Resolve]                       │
└─────────────────────────────────────────┘
```

**Features:**
- Comment on specific lines
- @mention teammates
- Thread discussions
- Resolve comments
- Comment history

### Activity Feed

```
┌─────────────────────────────────────────┐
│ Activity                                │
├─────────────────────────────────────────┤
│ Alice published v12              2h ago │
│ Bob modified Button.tsx          3h ago │
│ Carol viewed project             5h ago │
│ Alice resolved 3 comments        1d ago │
│ Bob joined the project           2d ago │
└─────────────────────────────────────────┘
```

### Version Control

**Built-in versioning:**
- Every AI edit creates a version
- Manual save points
- Named versions
- Compare any two versions
- Restore previous version

**GitHub Sync:**
- Two-way sync
- Auto-commit on publish
- Pull external changes
- Branch support
- PR creation

---

## Integrations

### Payment Integration

Add payments without code:

```
User: "Add subscription payments with monthly and yearly plans"

Agent:
1. Connects payment provider
2. Creates subscription products
3. Builds pricing page
4. Implements checkout flow
5. Adds billing portal
6. Sets up webhooks
```

**Features:**
- One-time payments
- Subscriptions
- Usage-based billing
- Multiple currencies
- Tax calculation
- Invoices
- Refunds

### E-commerce

Full e-commerce capabilities:

- Product catalog
- Inventory management
- Shopping cart
- Checkout flow
- Order management
- Shipping calculation
- Discount codes

### Email

Transactional and marketing email:

- Welcome emails
- Password reset
- Order confirmations
- Newsletter
- Email templates
- Delivery tracking

### Analytics Integration

Connect third-party analytics:

- Page views
- User events
- Conversion tracking
- Custom dashboards
- Real-time data

### Automation (n8n-style)

Connect to 400+ services:

```
Trigger: New user signup
   ↓
Action: Add to email list
   ↓
Action: Send Slack notification
   ↓
Action: Create CRM contact
```

### Custom API Integration

Agent can integrate any API:

```
User: "Integrate with the weather API"

Agent:
1. Reads API documentation
2. Creates API client
3. Adds environment variable for API key
4. Creates wrapper function
5. Uses in component
```

---

## Templates & Scaffolding

### Starter Templates

**Categories:**
- SaaS Applications
- E-commerce
- Landing Pages
- Dashboards
- Mobile Apps
- Portfolios
- Blogs
- Documentation

**Per template:**
- Live preview
- Feature list
- Required integrations
- One-click clone
- Customization guide

### Template Gallery

```
┌─────────────────────────────────────────┐
│ Templates                               │
├─────────────────────────────────────────┤
│ ┌─────────┐ ┌─────────┐ ┌─────────┐    │
│ │ SaaS    │ │ Store   │ │ Landing │    │
│ │ Starter │ │ Front   │ │ Page    │    │
│ └─────────┘ └─────────┘ └─────────┘    │
│                                         │
│ ┌─────────┐ ┌─────────┐ ┌─────────┐    │
│ │ Admin   │ │ Blog    │ │ Docs    │    │
│ │ Panel   │ │ Theme   │ │ Site    │    │
│ └─────────┘ └─────────┘ └─────────┘    │
└─────────────────────────────────────────┘
```

### Community Templates

User-created templates:

- Browse by category
- Sort by popularity
- Filter by features
- Preview before clone
- Creator profiles
- Ratings and reviews

### Custom Templates

Save your own templates:

- Save current project as template
- Private or public
- Share with team
- Sell on marketplace

---

## Developer Experience

### Command Palette

```
┌─────────────────────────────────────────┐
│ > _                                     │
├─────────────────────────────────────────┤
│ > Open File...                          │
│ > Run Command...                        │
│ > Go to Symbol...                       │
│ > Change Theme...                       │
│ > View Keyboard Shortcuts               │
└─────────────────────────────────────────┘
```

**Quick actions:**
- Cmd+K to open
- Fuzzy search
- Recently used
- Custom commands

### Keyboard Shortcuts

| Action | Mac | Windows |
|--------|-----|---------|
| Open file | ⌘P | Ctrl+P |
| Command palette | ⌘K | Ctrl+K |
| Save | ⌘S | Ctrl+S |
| Find | ⌘F | Ctrl+F |
| Find in project | ⌘⇧F | Ctrl+Shift+F |
| Toggle terminal | ⌘` | Ctrl+` |
| Publish | ⌘⇧P | Ctrl+Shift+P |

### Code Actions

**Quick fixes:**
- Import missing module
- Fix TypeScript error
- Convert to arrow function
- Extract component
- Rename symbol
- Add missing props

**Refactoring:**
- Extract function
- Inline variable
- Move to new file
- Convert class to function

### Snippets

Built-in snippets for common patterns:

```typescript
// Type: "rfc" + Tab
export function Component() {
  return (
    <div>
      |
    </div>
  );
}
```

**Custom snippets:**
- Create your own
- Share with team
- Language-specific
- Variable placeholders

### Extension System

Extend Vibe with plugins:

- Custom themes
- Additional languages
- Tool integrations
- Workflow automations

---

## Security & Compliance

### Security Scanning

Automatic scanning on every publish:

**Checks:**
- Dependency vulnerabilities
- Hardcoded secrets
- SQL injection risks
- XSS vulnerabilities
- CSRF protection
- Insecure HTTP usage
- Outdated packages

**Report:**
```
┌─────────────────────────────────────────┐
│ Security Scan                           │
├─────────────────────────────────────────┤
│ ✓ No critical vulnerabilities          │
│ ⚠ 2 moderate issues                     │
│   - Outdated lodash (4.17.20)           │
│   - Missing CSP header                  │
├─────────────────────────────────────────┤
│ [Fix Automatically] [Ignore] [Details]  │
└─────────────────────────────────────────┘
```

### Access Control

**Workspace level:**
- SSO integration (SAML, OIDC)
- IP allowlisting
- Session policies
- MFA requirement

**Project level:**
- Role-based access
- Environment restrictions
- Deploy permissions

### Audit Logs

Complete activity history:

```
┌─────────────────────────────────────────┐
│ Audit Log                               │
├─────────────────────────────────────────┤
│ 2024-12-10 14:30 alice@acme.com         │
│ Published version 12                    │
│                                         │
│ 2024-12-10 14:15 bob@acme.com           │
│ Modified src/App.tsx                    │
│                                         │
│ 2024-12-10 13:00 alice@acme.com         │
│ Added environment variable              │
└─────────────────────────────────────────┘
```

### Compliance

- **SOC 2 Type II** — Security certification
- **GDPR** — EU data protection
- **CCPA** — California privacy
- **HIPAA** — Healthcare (Enterprise)
- **Data residency** — Choose region

### Data Training Opt-Out

Enterprise users can opt out of having their project data used for AI training.

---

## Analytics & Insights

### Project Analytics

```
┌─────────────────────────────────────────┐
│ Analytics                               │
├─────────────────────────────────────────┤
│ Visitors: 1,234 (↑12%)                  │
│ Page Views: 4,567 (↑8%)                 │
│ Avg. Session: 2m 34s                    │
├─────────────────────────────────────────┤
│ Top Pages                               │
│ 1. /home (45%)                          │
│ 2. /pricing (23%)                       │
│ 3. /features (15%)                      │
├─────────────────────────────────────────┤
│ [Daily] [Weekly] [Monthly]              │
└─────────────────────────────────────────┘
```

**Metrics:**
- Unique visitors
- Page views
- Session duration
- Bounce rate
- Traffic sources
- Device breakdown
- Geographic distribution

### Usage Analytics

For authenticated apps:

- User signups
- Active users (DAU/MAU)
- Feature usage
- Conversion funnels
- Retention curves

### Real-Time Dashboard

Live view of current activity:

- Active users
- Current pages
- Live events
- Geographic map

---

## Mobile & Responsive

### Mobile Preview

Test on any device size:

- iPhone sizes
- iPad sizes
- Android common sizes
- Custom dimensions

### Touch Interactions

Design for mobile:

- Touch targets
- Swipe gestures
- Pull to refresh
- Bottom sheets

### PWA Support

Turn any Vibe app into a PWA:

- Add to home screen
- Offline support
- Push notifications
- App-like experience

---

## OpenAgents Ecosystem

### Nostr Identity

- Keypair generated on first run
- No email/password required
- Portable across OpenAgents apps
- Optional account linking

### Bitcoin/Lightning Payments

- Pay for credits with Bitcoin
- Instant Lightning settlement
- Self-custodial wallet
- Micropayment support

### Commander Integration

- Same identity
- Shared wallet
- Agent orchestration
- Swarm compute access

### Marketplace

- Discover agents
- Publish templates
- Sell your work
- Buy premium features

### ATIF Trajectories

- Universal action format
- Cross-tool compatibility
- Replay and debug
- Share and learn

---

## Roadmap Features

### Coming Soon

- [ ] Figma import
- [ ] Screenshot to code
- [ ] Voice commands
- [ ] Multi-language UI
- [ ] Mobile app editing

### Future

- [ ] AR/VR preview
- [ ] AI pair programming
- [ ] Automated testing
- [ ] Performance monitoring
- [ ] A/B testing built-in

---

*This document is a living specification. Features will be implemented iteratively based on user feedback and technical feasibility.*
