# Tool Protocol

A tool that a model can call to gather information at runtime or perform side effects.

**iOS 26.0+**
**iPadOS 26.0+**
**Mac Catalyst 26.0+**
**macOS 26.0+**
**visionOS 26.0+**

```swift
protocol Tool<Arguments, Output> : Sendable
```

### Mentioned In

*   Generating content and performing tasks with Foundation Models
*   Categorizing and organizing data with content tags
*   Expanding generation with tool calling

## Overview

Tool calling enables a model to execute your code to access current information, such as recent events or data from your application. A tool is defined by a `name` and a `description`, which are included in the prompt to help the model determine when and how to use the tool.

A `Tool` must implement a `call(arguments:)` method. This method accepts arguments that conform to the `ConvertibleFromGeneratedContent` protocol and returns an output that conforms to the `PromptRepresentable` protocol. This allows the model to process and reason about the tool's output in subsequent interactions. The `Output` is typically a `String` or another `Generable` type.

```swift
struct FindContacts: Tool {
    let name = "findContacts"
    let description = "Find a specific number of contacts"


    @Generable
    struct Arguments {
        @Guide(description: "The number of contacts to get", .range(1...10))
        let count: Int
    }


    func call(arguments: Arguments) async throws -> [String] {
        var contacts: [CNContact] = []
        // Fetch a number of contacts using the arguments.
        let formattedContacts = contacts.map {
            "\($0.givenName) \($0.familyName)"
        }
        return formattedContacts
    }
}
```

For concurrent execution by the framework, any tools you create must conform to the `Sendable` protocol. If the model needs to use the output of one tool as the input for another, it will perform tool calls sequentially.

You are in control of your tool's lifecycle, which allows you to manage its state between calls to the model. For instance, you could maintain a list of database records that you don't want to reuse in subsequent tool calls.

When you include a tool in your generation request, the tool's definitions (name, description, and parameter information) are added to the prompt. This contributes to the overall size of the context window. After your tool is called, its output is sent back to the model for additional processing.

### To efficiently use tool calling:

*   Keep `Guide(description:)` descriptions concise.
*   Limit the number of tools to between three and five.
*   Only include a tool when it is essential for the task.
*   If a tool is crucial, run it before calling the model and directly integrate its output into the prompt.

If your session exceeds the available context size, a `LanguageModelSession.GenerationError.exceededContextWindowSize(_:)` error will be thrown. In such cases, consider dividing tool calls across new `LanguageModelSession` instances. For more details on managing the context window size, refer to TN3193: Managing the on-device foundation model’s context window.

***

## Topics

### Invoking a tool

`func call(arguments: Self.Arguments) async throws -> Self.Output`
A language model will call this method when it needs to use this tool. (Required)

`associatedtype Arguments : ConvertibleFromGeneratedContent`
The arguments that this tool accepts. (Required)

`associatedtype Output : PromptRepresentable`
The output produced by this tool for the language model to use in subsequent interactions. (Required)

### Getting the tool properties

`var description: String`
A description in natural language of when and how to use the tool. (Required)

`var includesSchemaInInstructions: Bool`
If `true`, the model’s name, description, and parameters schema will be injected into the instructions of sessions that use this tool. (Required, Default implementation provided)

`var name: String`
A unique name for the tool, such as “get_weather”, “toggleDarkMode”, or “search contacts”. (Required, Default implementation provided)

`var parameters: GenerationSchema`
A schema for the parameters this tool accepts. (Required, Default implementation provided)

***

## Relationships

### Inherits From

*   `Sendable`
*   `SendableMetatype`

***

## See Also

### Tool calling

*   **Expanding generation with tool calling**: Create tools that allow the model to carry out tasks specific to your use case.
*   **Generate dynamic game content with guided generation and tools**: Enhance gameplay with AI-generated dialogue and encounters that are personalized to the player.

struct MyHistoryView: View {


    @State
    var session = LanguageModelSession(
        tools: [BreadDatabaseTool()]
    )

    var body: some View {
        List(session.transcript) { entry in
            switch entry {
            case .instructions(let instructions):
                // Display the instructions the model uses.
            case .prompt(let prompt):
                // Display the prompt made to the model.
            case .toolCall(let call):
                // Display the call details for a tool, like the tool name and arguments.
            case .toolOutput(let output):
                // Display the output that a tool provides back to the model.
            case .response(let response):
                // Display the response from the model.
            }
        }.task {
            do {
                try await session.respond(to: "Find a milk bread recipe.")
            } catch let error {
                // Handle the error.
            }
        }
    }

}
