# iOS: Job Creation & Submission

**Phase:** 2 - Payments
**Component:** iOS App
**Priority:** P0 (Critical - Enables buyers)
**Estimated Effort:** 2-3 weeks

## Summary

Implement job creation and submission UI in the iOS app, allowing users to create NIP-90 job requests, configure parameters, set bids, and submit to the marketplace.

## Motivation

iOS users need to **buy compute** by creating jobs. This enables the buyer side of the marketplace.

## Acceptance Criteria

### Job Creation UI (Pattern 3: Core Interactions + Pattern 5: Progressive Complexity)
- [ ] **Job kind selector** with clear categorization
  - Browse from Job Schema Registry (issue #004)
  - **Categories**: "Text", "Code", "Media", "Custom" (not raw kind numbers)
  - **Visual cards** with icons, not just dropdown list
  - **Popular jobs** section at top (summarize, translate, code review)
  - **Search** job kinds by keyword or description
- [ ] **Input field** with contextual help (Pattern 1: Onboarding)
  - **Placeholder text**: Show example based on selected job kind
    - Text summarization: "Paste article or URL to summarize..."
    - Code review: "Paste code or link to repository..."
  - **Input type auto-detection**: URL vs text vs file
  - **File upload**: Drag-and-drop area with clear file type/size limits
  - **Input validation**: Show character count, file size, format requirements
- [ ] **Parameter editor** (dynamic based on job schema)
  - **Smart defaults**: Pre-fill common parameters (don't make users configure everything)
  - **Collapsible sections**: Basic parameters (always visible) vs Advanced parameters (collapsed)
  - **Helper text**: Explain what each parameter does in plain language
  - **Real-time validation**: Show validation errors immediately, not on submit
  - **Parameter presets**: "Quick", "Balanced", "Quality" for complex params
- [ ] **Bid amount input** with clear pricing guidance (Pattern 5: Hide Complexity)
  - **Default suggestion**: "Recommended: 1000 sats" based on job kind
  - **Visual slider** for common amounts (100, 500, 1000, 5000 sats)
  - **Currency conversion**: Show both msats and sats (hide BTC conversion in Advanced)
  - **Price comparison**: "Similar jobs cost 500-2000 sats"
  - **Balance check**: Warn if bid exceeds wallet balance
- [ ] **Encryption toggle** with clear explanation (Pattern 1: Gradual Education)
  - **Default**: OFF (don't encrypt by default - adds complexity)
  - **Label**: "Private parameters" (not "Encrypt via NIP-04")
  - **Help text**: "Hide your parameters from others. Required for sensitive data."
  - **Trade-off warning**: "Encrypted jobs may have fewer providers"
- [ ] **Provider selection** (Pattern 5: Progressive Complexity)
  - **Default**: "Any provider" (broadcast to all)
  - **Show recommended providers** based on job kind and user history
  - **Provider cards**: Show name, success rate, price, turnaround time
  - **Advanced option**: "Choose specific provider" (collapsed by default)
- [ ] **Preview job request before submission** (Pattern 3: Clear Feedback)
  - **Summary card**: Job kind, input preview, parameters, bid, provider
  - **Edit buttons**: Quick access to edit each section
  - **Cost estimate**: "Total cost: ~1000 sats + network fees"
  - **Warnings**: Show any issues (low bid, no providers, etc.)

### Job Submission (Pattern 3: Core Interactions - Clear Visual Feedback)
- [ ] **Validate inputs** against job schema (issue #004)
  - **Real-time validation**: Show errors as user types (don't wait for submit)
  - **Clear error messages**: "Input too long (5000 chars max)" not "Validation error"
  - **Inline errors**: Show errors next to the field that failed
  - **Block submit**: Disable submit button until all errors resolved
- [ ] **Sign job request event** (NIP-90 kind:5000-5999)
  - **Loading state**: Show spinner with "Signing job request..."
  - **Biometric prompt**: If required by security settings
  - **Error handling**: "Signing failed. Try again." with retry button
- [ ] **Encrypt params** if requested (NIP-04 with provider pubkey)
  - **Loading state**: "Encrypting parameters..."
  - **Progress indicator**: Show percentage if large data
- [ ] **Publish to marketplace relays** with clear progress
  - **Multi-step progress**:
    1. "Signing job request..." (0-30%)
    2. "Publishing to relays..." (30-70%)
    3. "Waiting for confirmation..." (70-100%)
  - **Relay status**: Show which relays accepted the job (2 of 3 relays)
  - **Partial success**: "Published to 2 of 3 relays. Waiting for providers..."
- [ ] **Subscribe to feedback** (kind:7000) for submitted job
  - **Status updates**: Show feedback events as they arrive
  - **Push notifications**: "Your job is processing" (if enabled)
- [ ] **Subscribe to result** (kind:6000-6999)
  - **Auto-navigate**: Go to job detail view after submission
  - **Real-time updates**: Show status changes in job list
- [ ] **Handle payment-required response** (show BOLT11 invoice)
  - **Clear prompt**: "Provider requires payment: 1000 sats"
  - **Invoice display**: QR code + copy button + "Pay in wallet" button
  - **Timeout indicator**: "Invoice expires in 10 minutes"
  - **Payment confirmation**: "Payment sent! Waiting for provider..."
  - **Cancel option**: "Cancel job" if user doesn't want to pay

### Job Templates (Pattern 5: Progressive Complexity)
- [ ] **Save job as template** (reusable configs)
  - **Prompt after first submission**: "Save this job as a template?" (opt-in, not automatic)
  - **Template name**: User-friendly name (e.g., "Article summarizer")
  - **Template preview**: Show icon, name, job kind, parameters
  - **Edit template**: Allow user to modify saved templates
  - **Delete template**: Swipe to delete with confirmation
- [ ] **Load template** for quick submission
  - **Templates section**: Show saved templates at top of job creation screen
  - **One-tap loading**: Select template → auto-fill all fields
  - **Editable after load**: Users can modify template values before submit
  - **Visual distinction**: "From template: [name]" indicator
- [ ] **Pre-populated common jobs** (Pattern 1: Onboarding)
  - **Built-in templates**: 3-5 popular jobs (summarize, translate, code review)
  - **"Try it" button**: Quick way to test the marketplace
  - **Sample data**: Include example input so users can submit immediately
  - **Clear labeling**: "Template" badge to distinguish from user's saved templates
  - **Progressive disclosure**: Hide templates after user creates their own (80/20 rule)

## Technical Design

### UI Structure
```
Jobs/
├── CreateJobView            // Job creation form
├── JobKindPickerView        // Select job kind
├── JobParametersView        // Dynamic param editor
├── JobPreviewView           // Preview before submit
└── JobTemplatesView         // Saved templates
```

### Core Implementation

```swift
// CreateJobView.swift

struct CreateJobView: View {
    @StateObject private var viewModel = CreateJobViewModel()
    @State private var selectedJobKind: JobKind?
    @State private var inputText = ""
    @State private var bidAmount: Int64 = 1000  // msats
    @State private var encryptParams = false
    @State private var selectedProvider: String?  // nil = broadcast

    var body: some View {
        NavigationStack {
            Form {
                Section("Job Type") {
                    Picker("Job Kind", selection: $selectedJobKind) {
                        ForEach(JobKind.allCases) { kind in
                            Text(kind.displayName).tag(kind as JobKind?)
                        }
                    }
                }

                if let jobKind = selectedJobKind {
                    Section("Input") {
                        TextEditor(text: $inputText)
                            .frame(minHeight: 100)
                    }

                    // Dynamic parameters based on schema
                    JobParametersView(
                        jobKind: jobKind,
                        params: $viewModel.params
                    )

                    Section("Pricing") {
                        TextField("Bid (msats)", value: $bidAmount, format: .number)
                        Text("\(Millisatoshi(bidAmount).description)")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }

                    Section("Options") {
                        Toggle("Encrypt Parameters", isOn: $encryptParams)

                        Picker("Provider", selection: $selectedProvider) {
                            Text("Any Provider").tag(nil as String?)
                            ForEach(viewModel.providers) { provider in
                                Text(provider.npub).tag(provider.id as String?)
                            }
                        }
                    }

                    Section {
                        Button("Submit Job") {
                            Task {
                                await viewModel.submitJob(
                                    kind: jobKind,
                                    input: inputText,
                                    params: viewModel.params,
                                    bid: bidAmount,
                                    encrypt: encryptParams,
                                    providerPubkey: selectedProvider
                                )
                            }
                        }
                        .disabled(inputText.isEmpty)
                    }
                }
            }
            .navigationTitle("Create Job")
        }
    }
}

@MainActor
class CreateJobViewModel: ObservableObject {
    @Published var params: [String: String] = [:]
    @Published var providers: [Provider] = []
    @Published var submittedJobId: String?

    func submitJob(
        kind: JobKind,
        input: String,
        params: [String: String],
        bid: Int64,
        encrypt: Bool,
        providerPubkey: String?
    ) async {
        // Build job using JobBuilder (from issue #004)
        var builder = JobBuilder(kind: kind)
        builder.input(.text(input))
        for (key, value) in params {
            builder.param(key, value)
        }
        builder.bid(bid)
        builder.encrypted(encrypt)

        do {
            // Sign and publish
            let event = try builder.build(
                privateKey: /* from IdentityViewModel */,
                recipientPubkey: providerPubkey
            )

            // Broadcast to relays
            await nostrClient.broadcast(event)
            submittedJobId = event.id

            // Subscribe to feedback/result (track in issue #015)
        } catch {
            // Handle error
        }
    }
}
```

## Dependencies

- **Issue #001**: Nostr Client Library
- **Issue #004**: Job Schema Registry (JobBuilder, validation)
- **Issue #005**: iOS Nostr Identity (signing keys)
- **Issue #006**: iOS Marketplace Viewer (provider list)

## Apple Compliance

**ASRG 3.1.1/3.1.3**: Job submission is **not** a purchase of in-app content (it's requesting external compute service).
- ✅ Payment handled outside app (Phase 2: BOLT11 invoice payment)
- ✅ Results viewed outside app or as read-only data

**Future**: If results consumed in-app, may need to redirect payment to web/desktop per apple-terms-research.md guidance.

## Testing

- [ ] Create job with all param types
- [ ] Validate params against schema
- [ ] Sign and publish job request
- [ ] Encrypted params (NIP-04)
- [ ] Provider-specific jobs

## Success Metrics

- [ ] Submit 10+ jobs successfully
- [ ] Jobs appear on marketplace relays
- [ ] Providers respond with feedback
- [ ] UI validates inputs correctly

## Future Enhancements

- File upload for inputs (images, documents)
- Job chaining (use result as input to next job)
- Batch job submission
