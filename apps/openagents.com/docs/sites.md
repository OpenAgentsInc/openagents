# Research report: OpenAI **ChatGPT Sites / Sites in Codex**

**Report date:** June 4, 2026
**Source base:** official OpenAI launch, developer, help, pricing, terms, release-note, and subprocessor documentation available as of June 4, 2026.

## Executive summary

OpenAI’s early-June 2026 “Sites” launch is officially presented as **ChatGPT Sites** or **Sites in Codex**, a preview capability inside Codex that lets users ask Codex to create, save, deploy, and inspect hosted websites, web apps, dashboards, games, and lightweight internal tools. OpenAI announced it on **June 2, 2026** as part of a broader Codex expansion that also introduced role-specific plugins and annotations. ([OpenAI][1])

The core product idea is **prompt-to-hosted-app**: a user describes a site or app, Codex generates the implementation, and the result can be deployed to an OpenAI-hosted production URL without a separate deployment pipeline. OpenAI frames the initial use case around **business and enterprise workspace apps**, such as dashboards, planners, enablement hubs, onboarding portals, and shared team workspaces. ([OpenAI Developers][2])

Technically, Sites is not just static-page generation. OpenAI’s docs describe support for lightweight **full-stack JavaScript/TypeScript web apps**, Cloudflare Worker-compatible ES module output, durable relational storage through **D1**, object/file storage through **R2**, workspace-authenticated identity, environment variables and secrets, saved versions, deployments, and access controls. ([OpenAI Help Center][3])

The product is still a **preview / beta-style service**. Pricing is free during preview, future pricing is not yet published, and several governance caveats matter: admins control availability in eligible workspaces, Sites data is not eligible for OpenAI data or inference residency at launch, and OpenAI’s Sites Terms place responsibility for site legality, privacy compliance, end-user handling, and support on the site creator. ([OpenAI Developers][4])

My overall assessment: **Sites is OpenAI’s first serious move from “AI coding assistant” toward “AI-native internal app platform.”** It overlaps with low-code tools, app builders, internal-tool platforms, and AI website builders, but its differentiator is tight integration with Codex, ChatGPT workspaces, OpenAI-hosted deployment, identity, and workspace governance.

---

## 1. Product overview

**ChatGPT Sites** is a Codex plugin that allows users to build and deploy hosted web experiences from inside Codex. The launch article describes Sites as a way to create “interactive websites and apps” that can be shared using a workspace URL, while the developer guide says Sites lets Codex “create, save, deploy, and inspect” websites, web apps, and games hosted by OpenAI. ([OpenAI][1])

The product sits inside OpenAI’s broader June 2026 Codex strategy. OpenAI positioned Codex as expanding beyond software engineering into different roles and workflows, with role-specific plugins, annotations, and Sites. OpenAI also said Codex had more than five million weekly users at the time of the announcement. ([OpenAI][1])

The initial product framing is **workspace-first**, not consumer website publishing. OpenAI’s Enterprise/Edu release notes describe ChatGPT Sites as a preview for eligible workspaces that lets users create, iterate on, and deploy lightweight full-stack JavaScript/TypeScript web apps with hosted URLs, Sign in with ChatGPT access, and data/file storage. ([OpenAI Help Center][3])

OpenAI’s availability language is slightly staged across official pages. The developer and help docs emphasize eligible **Business and Enterprise** workspaces, with Business enabled by default and Enterprise controlled through admin settings; the Enterprise/Edu release notes also describe availability for eligible **Enterprise and Edu** workspaces. The safest reading is that Sites is rolling out by workspace plan and admin eligibility, not as a universal consumer feature. ([OpenAI Developers][2])

---

## 2. What users can build

OpenAI describes Sites as suitable for internal dashboards, planners, shared team workspaces, lightweight tools, and hosted apps that can update as needs change. The launch page gives examples such as dashboards and planners, while the developer guide gives prompt examples for dashboards, internal tools, persistent player scores, and avatar uploads. ([OpenAI][1])

The official showcase pages make the intended product category clearer. OpenAI’s Sites examples include an **Onboarding Hub**, **Enablement Hub**, **Pulse Dashboard**, **Sparkboard**, **Launch Cal**, and **Event Planning Hub**, all framed as internal or workspace-oriented apps built with Sites in Codex. ([OpenAI Developers][5])

Three representative examples:

**Onboarding Hub**: an internal onboarding site that uses connected workspace context such as Notion, Drive, and Slack, persists onboarding checklist state and notes in D1, and uses R2 for uploaded screenshots or policy PDFs. OpenAI lists it as built with Sites in Codex, GPT-5.5, and Vinext. ([OpenAI Developers][6])

**Pulse Dashboard**: a dashboard for team metrics and updates that can pull context from tools like Slack and spreadsheets, then persist dashboard configurations, saved filters, annotations, and cached metric snapshots in D1. OpenAI again lists Sites in Codex, GPT-5.5, and Vinext in the build details. ([OpenAI Developers][7])

**Sparkboard**: an idea board for workspace suggestions, votes, comments, status history, and leaderboards. Its official build notes mention D1 tables, duplicate-vote prevention through a unique index, a Cloudflare Worker-compatible ES module build, `.openai/hosting.json`, and a D1 binding named `DB`. ([OpenAI Developers][8])

---

## 3. User workflow

The basic workflow is:

1. **Enable or access the Sites plugin in Codex.** In eligible workspaces, users can add the Sites plugin in the Codex app or invoke it in a prompt with `@Sites`. Admins may need to enable the capability depending on workspace plan and controls. ([OpenAI Developers][2])

2. **Describe the desired app.** The user can prompt Codex to create a new site, dashboard, game, internal tool, or to adapt an existing compatible project into a hosted Site. OpenAI’s developer guide includes examples such as creating a customer success dashboard with sign-in, deploying an existing Vite app, or adding durable player scores and avatar uploads. ([OpenAI Developers][2])

3. **Codex generates and validates the project.** Codex is expected to validate that the project builds before it is saved or deployed. Users can ask Codex to save a deployable version for review or deploy an already approved saved version. ([OpenAI Developers][2])

4. **Save a version, then deploy.** OpenAI distinguishes between saving a deployable version and deploying it. A saved version is associated with the source Git commit; a deployment publishes a saved version and returns a production URL. OpenAI warns that every deployment URL is production, so teams should save for review before going live. ([OpenAI Developers][2])

5. **Manage versions, status, and access.** The Sites sidebar lets users return to a project, inspect saved versions, check deployment status, and change access settings. ([OpenAI Developers][2])

This workflow is important because Sites is not merely “generate HTML.” It is closer to a guided build-and-deploy loop where Codex acts as product manager, developer, build assistant, and deployment operator inside the same product surface.

---

## 4. Technical architecture

### 4.1 Application model

OpenAI describes Sites as supporting lightweight full-stack JavaScript/TypeScript web apps. The developer guide says existing projects must produce a **Cloudflare Worker-compatible output as ES modules**, and the official examples repeatedly reference Vinext-based starters and Worker-compatible builds. ([OpenAI Help Center][3])

This implies a deployment model designed for edge-style server runtimes rather than traditional long-running servers. OpenAI has not published a complete runtime topology, quota model, cold-start model, or networking specification, so the safest technical characterization is: **OpenAI-hosted, Cloudflare Worker-compatible web app deployment, with Cloudflare infrastructure involved in the hosting stack.** OpenAI’s subprocessor list names Cloudflare for CDN and web hosting, and specifically defines web hosting as hosting ChatGPT Sites-created web pages. ([OpenAI][9])

### 4.2 Project metadata and deployment binding

Sites projects use a local metadata file at:

```json
{
  "project_id": "<project-id>",
  "d1": "DB",
  "r2": null
}
```

OpenAI’s docs describe `.openai/hosting.json` as storing the linkage between a local source tree and the hosted Sites project, along with optional storage binding names. The `project_id` links the local code to the hosted project, while `d1` and `r2` specify database and object-storage bindings. ([OpenAI Developers][2])

This is an important architectural choice. It means the app’s source code and deployment metadata are partly local/repo-based, while runtime resources such as secrets, storage bindings, saved versions, access mode, and production deployments are managed through OpenAI’s Sites hosting layer.

### 4.3 Build and deployment lifecycle

OpenAI defines a two-stage lifecycle:

**Save version**: Codex builds a deployable version and associates it with the source Git commit.

**Deploy**: a saved version is published and receives a production URL.

OpenAI explicitly says every deployment URL is production, so the recommended workflow is to save a version for review before deploying it. ([OpenAI Developers][2])

This design resembles deployment systems such as Vercel, Netlify, Cloudflare Pages/Workers, or internal platform-as-a-service tools, but with the authoring loop handled by Codex.

### 4.4 Storage layer: D1 and R2

OpenAI’s docs expose two durable storage concepts:

**D1**: relational database storage for structured durable data.

**R2**: object storage for files, uploads, and binary objects.

OpenAI recommends D1 for durable structured state, R2 for uploaded files, and D1 plus R2 when an app needs file uploads with metadata. It also says developers should not request durable storage for temporary UI state such as presentation-only filters. ([OpenAI Developers][2])

The showcase examples confirm the intended pattern. Onboarding Hub uses D1 for checklists, notes, bookmarks, and onboarding state, and R2 for uploaded screenshots or PDFs. Pulse Dashboard uses D1 for dashboard configurations, saved filters, metric annotations, and cached snapshots. Sparkboard uses D1 for ideas, votes, comments, status history, and leaderboard snapshots. ([OpenAI Developers][6])

### 4.5 Identity and authentication

Sites supports **Sign in with ChatGPT** and workspace-authenticated access. OpenAI’s Enterprise/Edu release notes describe Sites apps as having hosted URLs, Sign in with ChatGPT access, and data/file storage. The help docs say Sites can be used to deploy workspace-internal web apps with Sign in with ChatGPT access and invite users in the same workspace. ([OpenAI Help Center][3])

OpenAI’s developer guide also mentions using workspace-authenticated user identity and, for authentication-enabled Sites projects, public sign-in or an external identity provider. However, the public-facing distribution model appears early and staged; the launch and help documentation primarily emphasize workspace-internal use. ([OpenAI Developers][2])

### 4.6 Access modes

OpenAI documents three access modes:

`admins_only`: only the site owner and workspace admins.

`workspace_all`: all active users in the workspace.

`custom`: specific users or groups.

OpenAI recommends keeping Sites limited to the owner/admins until review is complete and setting the intended audience before sharing. ([OpenAI Developers][2])

This is a key difference from ordinary public website builders. The first-class access model is workplace governance, not anonymous public traffic.

### 4.7 Secrets and environment variables

OpenAI says environment variables and secrets are managed in the Sites panel, not in `.openai/hosting.json`. Developers should not commit secrets, may use `.env` files only for local development, and need to redeploy after changing runtime secrets. ([OpenAI Developers][2])

This makes Sites closer to a managed app platform than a static-site generator. It also means organizations need secret-management procedures, code review, and deployment ownership even if non-engineers can prompt Codex to build the app.

### 4.8 Review checklist

OpenAI’s developer guide recommends reviewing source changes, database migrations, build results, saved versions, intended audience, secret configuration, deployment status, and production URL before sharing. ([OpenAI Developers][2])

That checklist reveals OpenAI’s own view of Sites: it is capable enough to require software-release discipline. Teams should treat Sites deployments as real internal apps, not throwaway mockups.

---

## 5. Admin, governance, and enterprise controls

Workspace controls are central to Sites. OpenAI says plugin access follows workspace app controls, and Enterprise/Edu admins can manage availability using role-based access control. Admins and owners can enable Sites from workspace settings, manage permissions and roles, and disable published sites from workspace settings. ([OpenAI Help Center][10])

The help documentation says Business workspaces have Sites enabled by default, while Enterprise availability is controlled through an Early Access toggle. It also says admins and owners have default access to all sites created by workspace members and can disable an existing site from workspace settings. ([OpenAI Help Center][10])

Codex usage information, including local clients and web/cloud delegated usage, is available through OpenAI’s Compliance API, according to the help docs. This matters for enterprises that need auditability, though OpenAI’s documentation distinguishes general Codex log coverage from cloud-task-specific endpoints. ([OpenAI Help Center][10])

---

## 6. Data, residency, security, and compliance

### 6.1 Training and workspace data treatment

OpenAI’s help docs state that Business, Enterprise, and Edu workspaces are not used by default to train OpenAI models on inputs and outputs. That is consistent with broader ChatGPT workspace data-control positioning, but organizations should still review the specific workspace settings, connector settings, and Sites terms before deploying sensitive apps. ([OpenAI Help Center][10])

### 6.2 Data residency limitation

A major launch limitation is data residency. OpenAI’s Data Residency documentation says **Codex and ChatGPT Sites are not eligible for data or inference residency at launch**. For Sites, OpenAI explicitly includes deployed Sites, site code, D1/R2 data or file storage, generated artifacts, and related logs in the non-eligible category. ([OpenAI Help Center][11])

This is likely one of the most important enterprise caveats. Organizations with strict geographic data-residency obligations should not assume Sites inherits their broader ChatGPT data residency configuration.

### 6.3 Subprocessors and hosting

OpenAI’s subprocessor list includes Cloudflare for CDN and web hosting. It defines web hosting as hosting ChatGPT Sites-created web pages and says applicable subprocessors may run security and safety classifiers on web pages and share results with OpenAI. ([OpenAI][9])

This supports the technical reading that Sites involves Cloudflare-compatible and Cloudflare-hosted infrastructure, but it does not by itself disclose every detail of the runtime, storage location, isolation model, or scaling behavior.

### 6.4 Legal responsibility and prohibited uses

OpenAI’s ChatGPT Sites Terms say the user owns the website content, while granting OpenAI a license to host, store, reproduce, modify, display, and otherwise operate the site content as needed for the service. The terms also say OpenAI may display attribution such as “powered by ChatGPT.” ([OpenAI][12])

The same terms place responsibility for Sites and site content on the user, including end users, functionality, legal compliance, warranties, and support. They prohibit uses including malware, surveillance or malicious code, targeting children under 13 or below the digital-consent age, money transfers, crypto transfers, and financial or investment transactions. ([OpenAI][12])

For privacy, the Sites Terms say that if a site collects or processes personal data, the creator is responsible for handling that data and providing a privacy policy where required. The terms also state that site creators are the data controller for end-user data and that users must not collect or process PHI or PCI data through Sites. ([OpenAI][12])

### 6.5 Beta-service risk

OpenAI’s Service Terms classify beta services as offered “as-is,” with no guarantee that they will become generally available, uninterrupted, error-free, secure, or free from loss or damage. OpenAI’s Sites Terms also state that OpenAI may remove, unpublish, delete, or disable a site at any time for any reason. ([OpenAI][13])

For production use, that means Sites should currently be treated as a preview platform for internal tools, prototypes, pilots, and lower-risk workflows unless an organization has explicitly accepted the beta risk.

---

## 7. Pricing and plan availability

OpenAI’s Codex pricing page says Sites is **free while in preview** and that pricing information will be available soon. OpenAI’s ChatGPT Rate Card repeats that ChatGPT Sites is currently available as a Codex plugin in preview and that pricing information is forthcoming. ([OpenAI Developers][4])

This means there is no stable long-term pricing model yet for app hosting, storage, bandwidth, database usage, object storage, build minutes, seats, or deployment volume. Any buyer evaluating Sites should treat current economics as temporary.

Availability is plan- and workspace-dependent. Official docs mention eligible Business, Enterprise, and, in Enterprise/Edu release notes, Edu workspaces. Business appears enabled by default in the help docs, while Enterprise and Edu access is governed by admin settings and RBAC. ([OpenAI Help Center][10])

---

## 8. Competitive and strategic implications

Sites changes the shape of Codex. Before Sites, Codex was primarily a coding agent that produced code, reviewed code, or helped operate development workflows. With Sites, Codex can produce a deployed artifact: a running app with identity, storage, hosting, access control, and a production URL. That moves OpenAI closer to the territory of internal-tool builders, low-code platforms, app-hosting providers, and AI website builders.

OpenAI appears to be positioning Sites as both a native capability and an ecosystem bridge. The launch page mentions partners including Vercel, Wix, Base44, Replit, Lovable, Figma, Webflow, and Emergent, suggesting OpenAI wants Codex to work across existing product-design and deployment ecosystems rather than replace all of them immediately. ([OpenAI][1])

The most defensible near-term use case is **internal workspace software**: dashboards, onboarding hubs, sales enablement tools, campaign calendars, team trackers, planning tools, lightweight CRUD apps, and authenticated internal portals. These are exactly the types of applications that are often too small for a formal engineering roadmap but too important for spreadsheets and slide decks.

The longer-term strategic question is whether OpenAI can make Sites safe and governable enough for enterprise production workloads. The core UX is powerful, but enterprise adoption will depend on pricing, runtime limits, audit depth, data residency, compliance certifications, app lifecycle controls, and how well admins can monitor and disable risky deployments.

---

## 9. Key limitations and open questions

**Data residency is not available at launch.** OpenAI explicitly excludes ChatGPT Sites, deployed Sites, code, D1/R2 storage, artifacts, and logs from data and inference residency eligibility. ([OpenAI Help Center][11])

**Pricing is unknown after preview.** Sites is free during preview, but OpenAI has not published future pricing for hosting, storage, bandwidth, or app usage. ([OpenAI Developers][4])

**Runtime limits are not fully disclosed.** OpenAI says the output must be Cloudflare Worker-compatible ES modules and shows D1/R2 bindings, but public docs do not yet disclose all limits for CPU time, memory, request volume, database size, object storage, bandwidth, cron jobs, background jobs, networking, or custom domains. ([OpenAI Developers][2])

**Public distribution is still ambiguous.** OpenAI’s terms mention possible future OpenAI-owned subdomains such as `[website-name].chatgpt.site`, and the developer guide mentions public sign-in for authentication-enabled projects, but most launch, help, and release-note language emphasizes workspace-internal apps. ([OpenAI][12])

**Legal and privacy burden sits heavily on the creator.** The creator is responsible for site content, legal compliance, support, personal-data handling, privacy notices, and avoiding prohibited categories such as PHI, PCI, financial transactions, crypto transfers, malware, and child-directed services. ([OpenAI][12])

**Beta-service risk is material.** OpenAI can remove or disable Sites, and beta services are provided without guarantees of general availability or uninterrupted, error-free, secure operation. ([OpenAI][13])

---

## 10. Practical evaluation checklist for teams

Before adopting Sites, an organization should answer five questions.

**First, what class of apps are allowed?** Sites is well suited for internal tools, dashboards, and workflow hubs. It is less clearly suited for regulated workflows, high-availability customer-facing systems, payment flows, healthcare data, or apps with strict residency requirements.

**Second, who can create and deploy?** Admins should decide whether Sites is available to all eligible users or only specific roles and groups. Access mode should default to `admins_only` during review, then move to `workspace_all` or `custom` only after approval. ([OpenAI Developers][2])

**Third, what data can be stored?** Teams need clear rules for D1 and R2 usage, especially because Sites storage and logs are not eligible for data residency at launch and PHI/PCI are prohibited by the Sites Terms. ([OpenAI Help Center][11])

**Fourth, what release process is required?** OpenAI’s own docs recommend reviewing source changes, migrations, build results, versions, audience, secrets, and production URL before sharing. That should become a lightweight internal release checklist. ([OpenAI Developers][2])

**Fifth, what happens when preview pricing changes?** Since pricing after preview is not yet published, teams should avoid building business-critical workflows that assume preview economics will continue. ([OpenAI Developers][4])

---

## Bottom line

ChatGPT Sites is best understood as **Codex plus managed app hosting**: an AI agent that can generate, revise, save, and deploy workspace apps with durable data, file storage, identity, access controls, and production URLs. The technical foundation—Worker-compatible JavaScript/TypeScript apps, D1, R2, `.openai/hosting.json`, secrets, saved versions, and workspace access modes—makes it substantially more capable than a simple AI website generator.

At the same time, it is still a preview product with unresolved questions around pricing, runtime limits, public distribution, data residency, and production guarantees. For now, its highest-value use case is fast creation of governed internal apps that would otherwise live in spreadsheets, docs, or backlogged engineering tickets.

[1]: https://openai.com/index/codex-for-every-role-tool-workflow/ "Codex for every role, tool, and workflow | OpenAI"
[2]: https://developers.openai.com/codex/sites "Sites – Codex | OpenAI Developers"
[3]: https://help.openai.com/en/articles/10128477-chatgpt-enterprise-edu-release-notes "ChatGPT Enterprise & Edu - Release Notes | OpenAI Help Center"
[4]: https://developers.openai.com/codex/pricing "Pricing – Codex | OpenAI Developers"
[5]: https://developers.openai.com/showcase/sites "Sites | OpenAI Developers"
[6]: https://developers.openai.com/showcase/onboarding-hub "OpenAI showcase - Onboarding Hub"
[7]: https://developers.openai.com/showcase/pulse-dashboard "OpenAI showcase - Pulse Dashboard"
[8]: https://developers.openai.com/showcase/idea-intake "OpenAI showcase - Sparkboard"
[9]: https://openai.com/policies/sub-processor-list/ "OpenAI Sub-processor list | OpenAI"
[10]: https://help.openai.com/en/articles/11369540-using-codex-with-your-chatgpt-plan "Using Codex with your ChatGPT plan | OpenAI Help Center"
[11]: https://help.openai.com/en/articles/9903489-data-residency-and-inference-residency-for-chatgpt "Data residency and inference residency for ChatGPT | OpenAI Help Center"
[12]: https://openai.com/policies/chatgpt-sites-terms/ "ChatGPT Sites Terms | OpenAI"
[13]: https://openai.com/policies/service-terms/ "Service terms | OpenAI"
