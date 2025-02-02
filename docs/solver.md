# OpenAgents Solver Documentation

[previous content unchanged until environment variables section...]

The main function:

1. Initializes logging
2. Parses command line arguments
3. Loads environment variables (GITHUB_TOKEN, OLLAMA_URL)
4. Initializes the GitHub service
5. Fetches issue details
6. Generates repository map
7. Orchestrates the planning and solution phases

[rest of content unchanged until Configuration section...]

## Configuration

The solver requires:

1. `GITHUB_TOKEN`: For GitHub API access
2. `OLLAMA_URL`: For model access (default: http://localhost:11434)

[rest of the document unchanged...]