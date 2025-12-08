# Guided Generation with Apple Foundation Models

Guided generation is the most powerful feature of our Foundation Models integration. It constrains model output to a specific schema using **constrained sampling**, guaranteeing valid structured output every time.

## Overview

### The Problem with Unguided Generation

When asking an LLM to generate structured data (like JSON), common problems include:

| Problem | Example |
|---------|---------|
| **Malformed JSON** | Missing brackets, trailing commas |
| **Wrong field names** | `test_category` instead of `category` |
| **Invalid enum values** | `"basic"` instead of `"happy_path"` |
| **Out-of-range numbers** | Confidence of 1.5 when max is 1.0 |
| **Missing required fields** | No `id` field on test cases |
| **Inconsistent structure** | Some tests have `reasoning`, others don't |

### The Solution: Guided Generation

Apple's Foundation Models framework provides `@Generable` and `@Guide` macros that define schemas. The model uses **constrained sampling** - it can only generate tokens that produce valid output.

```swift
@Generable(description: "A test case")
struct GeneratedTest: Codable {
    var id: String

    @Guide(description: "Test category", .anyOf([
        "happy_path", "boundary", "edge_case"
    ]))
    var category: String  // Can ONLY be one of these values

    @Guide(description: "Confidence", .range(0.0...1.0))
    var confidence: Float  // ALWAYS between 0 and 1
}
```

### Performance Comparison

We benchmarked test generation for the `regex-log` task:

| Metric | Unguided | Guided | Improvement |
|--------|----------|--------|-------------|
| **Duration** | ~93s | ~18-22s | **4x faster** |
| **JSON validity** | ~90% | 100% | No retries needed |
| **Category accuracy** | 0% (all "happy_path") | 100% (proper distribution) | Correct taxonomy |
| **Confidence range** | Sometimes > 1.0 | Always 0.0-1.0 | Bounded values |

#### Category Distribution

**Unguided (all same):**
```
Categories: happy_path:23
```

**Guided (proper distribution):**
```
Categories: existence:1, happy_path:4, boundary:2, edge_case:2, invalid_input:3
```

## How It Works

### 1. Define the Schema (Swift)

Create a struct with `@Generable` macro in `GuidedTypes.swift`:

```swift
import FoundationModels

@Generable(description: "A single test case for verifying code correctness")
struct GeneratedTest: Codable {
    var id: String

    @Guide(description: "The test input (e.g., log line for regex-log)")
    var input: String

    @Guide(description: "Expected output or null if no match expected")
    var expectedOutput: String?

    @Guide(description: "Why this test is important")
    var reasoning: String

    @Guide(description: "Test category", .anyOf([
        "existence",      // File/output created
        "format",         // Structure valid
        "happy_path",     // Basic correct behavior
        "boundary",       // Min/max limits
        "edge_case",      // Tricky scenarios
        "invalid_input",  // Should fail/reject
        "integration"     // System-level
    ]))
    var category: String

    @Guide(description: "Confidence score 0-1", .range(0.0...1.0))
    var confidence: Float
}

@Generable(description: "Generated test suite from task description")
struct TestGenerationResult: Codable {
    @Guide(description: "Requirements extracted from task description")
    var requirements: [String]

    @Guide(description: "Assumptions made during generation")
    var assumptions: [String]

    @Guide(description: "Areas of uncertainty")
    var uncertainties: [String]

    @Guide(description: "Generated test cases")
    var tests: [GeneratedTest]
}
```

### 2. Add Handler Case (Swift)

In `ChatHandler.swift`, add a case for your schema:

```swift
private func handleGuidedGeneration(
    session: LanguageModelSession,
    prompt: String,
    responseFormat: ResponseFormatRequest
) async throws -> String {
    if let schemaType = responseFormat.schemaType {
        switch schemaType {
        case "test_generation":
            let response = try await session.respond(
                to: prompt,
                generating: TestGenerationResult.self
            )
            return encodeToJSON(response.content)

        // Add more cases here...
        default:
            throw FMError.invalidRequest("Unknown schema type: \(schemaType)")
        }
    }
    // ...
}
```

### 3. Request with response_format (TypeScript)

```typescript
const response = yield* fm.chat({
  messages: [{ role: "user", content: prompt }],
  temperature: 0.3,
  maxTokens: 4096,
  responseFormat: {
    type: "json_schema",
    schema_type: "test_generation",  // Matches Swift case
  },
});

// Response content is GUARANTEED to be valid JSON matching TestGenerationResult
const result = JSON.parse(response.choices[0].message.content);
```

## Guide Constraints

The `@Guide` macro supports several constraint types:

### `.anyOf()` - Enum Values

Constrain to specific string values:

```swift
@Guide(description: "Status", .anyOf(["pending", "running", "completed", "failed"]))
var status: String
```

### `.range()` - Numeric Bounds

Constrain numbers to a range:

```swift
@Guide(description: "Score", .range(0...100))
var score: Int

@Guide(description: "Probability", .range(0.0...1.0))
var probability: Float
```

### Arrays

Arrays of Generable types:

```swift
@Guide(description: "List of items")
var items: [ItemType]  // ItemType must also be @Generable
```

### Optional Fields

Use `?` for optional fields:

```swift
@Guide(description: "Optional note")
var note: String?  // Can be null in JSON
```

## Available Schema Types

Currently implemented schemas:

| Schema Type | Description | Swift Type |
|-------------|-------------|------------|
| `test_generation` | Generate test cases from task descriptions | `TestGenerationResult` |

## Adding New Schema Types

### Step 1: Define Swift Types

Add to `swift/foundation-bridge/Sources/foundation-bridge/GuidedTypes.swift`:

```swift
@Generable(description: "Your description here")
struct YourOutputType: Codable {
    var requiredField: String

    @Guide(description: "Description", .anyOf(["a", "b", "c"]))
    var enumField: String

    @Guide(description: "Description", .range(0.0...1.0))
    var numericField: Float

    var optionalField: String?
}
```

### Step 2: Add Handler Case

In `ChatHandler.swift`:

```swift
case "your_schema_type":
    let response = try await session.respond(
        to: prompt,
        generating: YourOutputType.self
    )
    return encodeToJSON(response.content)
```

### Step 3: Rebuild Bridge

```bash
cd swift/foundation-bridge
swift build
cp .build/debug/foundation-bridge ../../bin/
```

### Step 4: Use from TypeScript

```typescript
interface YourOutputType {
  requiredField: string;
  enumField: "a" | "b" | "c";
  numericField: number;
  optionalField?: string;
}

const response = yield* fm.chat({
  messages: [{ role: "user", content: prompt }],
  responseFormat: {
    type: "json_schema",
    schema_type: "your_schema_type",
  },
});

const result: YourOutputType = JSON.parse(response.choices[0].message.content);
```

## Best Practices

### 1. Keep Descriptions Concise

The model sees descriptions in the prompt. Keep them short:

```swift
// Good
@Guide(description: "Test category")

// Too verbose
@Guide(description: "The category of this test case which should describe what aspect of the system is being tested")
```

### 2. Use Specific Enum Values

Be explicit about allowed values:

```swift
// Good - explicit values
@Guide(description: "Priority", .anyOf(["low", "medium", "high", "critical"]))

// Bad - vague
var priority: String  // Model might generate anything
```

### 3. Match TypeScript Types

Keep TypeScript types in sync with Swift:

```swift
// Swift
@Guide(description: "Score", .range(0...100))
var score: Int
```

```typescript
// TypeScript
interface Result {
  score: number;  // Will always be 0-100
}
```

### 4. Test Schema Changes

After modifying schemas, test with a simple prompt:

```bash
bun run src/hillclimber/test-gen-cli.ts --task regex-log --model local
```

Check that:
- All fields are present
- Enum values are valid
- Numbers are in range
- Arrays have correct item types

## Debugging

### Invalid Schema Type

```
Error: Unknown schema type: foo
```

The `schema_type` doesn't match any case in `ChatHandler.swift`. Add a new case or use an existing type.

### Codable Errors

```
Error: Type 'YourType' does not conform to protocol 'Codable'
```

All `@Generable` types must also conform to `Codable`:

```swift
@Generable(description: "...")
struct YourType: Codable {  // Add Codable
    // ...
}
```

### Compilation Errors

```
Error: Instance method 'respond' requires that 'YourType' conform to 'Generable'
```

The type needs the `@Generable` macro:

```swift
@Generable(description: "...")  // Add this
struct YourType: Codable {
    // ...
}
```

## Comparison with Other Approaches

### vs. Prompt Engineering

| Approach | Reliability | Speed | Complexity |
|----------|-------------|-------|------------|
| Prompt engineering | ~80-90% | Baseline | Low |
| Retry on parse error | ~95% | Slower | Medium |
| Guided generation | **100%** | **Faster** | Medium |

### vs. Tool Calling

Tool calling is for invoking actions. Guided generation is for structured output.

| Use Case | Approach |
|----------|----------|
| "Call the weather API" | Tool calling |
| "Generate a JSON config" | Guided generation |
| "List test cases as structured data" | Guided generation |

## Example Output

Test generation for `regex-log` with guided generation:

```json
{
  "requirements": [
    "Regex must correctly match dates in YYYY-MM-DD format in lines with IPv4 addresses.",
    "Valid dates must be matched, ignoring any preceding or following alphanumeric characters.",
    "Regex must handle multiple dates per line by matching only the last date."
  ],
  "assumptions": [
    "Regex pattern is correctly implemented in /app/regex.txt.",
    "Python's re.findall is correctly used with re.MULTILINE flag."
  ],
  "uncertainties": [
    "Regex might not handle edge cases like excessively long dates gracefully.",
    "The regex might not handle non-ASCII characters in the log file correctly."
  ],
  "tests": [
    {
      "id": "happy_path_1",
      "input": "192.168.1.1 - 2023-02-29: Log entry 1",
      "expectedOutput": "['2023-02-29']",
      "reasoning": "Tests basic valid input with a single date.",
      "category": "happy_path",
      "confidence": 0.95
    },
    {
      "id": "boundary_min",
      "input": "192.168.1.3 - 2021-01-01: Log entry 3",
      "expectedOutput": "['2021-01-01']",
      "reasoning": "Tests minimum valid date boundary.",
      "category": "boundary",
      "confidence": 0.95
    },
    {
      "id": "invalid_input_empty",
      "input": "",
      "expectedOutput": null,
      "reasoning": "Tests empty log line input.",
      "category": "invalid_input",
      "confidence": 0.95
    }
  ]
}
```

Note:
- `category` values are exactly from the allowed set
- `confidence` is always 0.0-1.0
- Structure is guaranteed valid

---

*Last updated: 2025-12-08*
