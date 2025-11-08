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

### Job Creation UI
- [ ] Job kind selector (browse from Job Schema Registry)
- [ ] Input field (text, URL, or file upload)
- [ ] Parameter editor (dynamic based on job schema)
- [ ] Bid amount input (msats, with sat/BTC conversion)
- [ ] Encryption toggle (encrypt params via NIP-04)
- [ ] Provider selection (choose specific provider or broadcast)
- [ ] Preview job request before submission

### Job Submission
- [ ] Validate inputs against job schema (issue #004)
- [ ] Sign job request event (NIP-90 kind:5000-5999)
- [ ] Encrypt params if requested (NIP-04 with provider pubkey)
- [ ] Publish to marketplace relays
- [ ] Subscribe to feedback (kind:7000) for submitted job
- [ ] Subscribe to result (kind:6000-6999)
- [ ] Handle payment-required response (show BOLT11 invoice)

### Job Templates
- [ ] Save job as template (reusable configs)
- [ ] Load template for quick submission
- [ ] Pre-populated common jobs (summarize, translate, code review)

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
