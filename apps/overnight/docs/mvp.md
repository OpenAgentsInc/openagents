# Overnight Coding Agent MVP Specification

## Overview
The Overnight Coding Agent is a terminal-based application built with the Effect framework that runs autonomously to process GitHub issues. It analyzes issues, adds comments, and creates pull requests without human intervention, running continuously until manually terminated.

## Core Functionality

1. **GitHub API Integration**
   - Connect to GitHub API using Octokit
   - Configure authentication with tokens
   - Handle API rate limiting

2. **Issue Processing Pipeline**
   - Fetch open issues from configured repositories
   - Filter issues based on configurable criteria (labels, age, etc.)
   - Process issues sequentially with proper error handling

3. **Analysis Engine**
   - Parse issue content and context
   - Determine appropriate action (comment, PR, skip)
   - Track processed issues to avoid duplication

4. **Response Actions**
   - Add comments to issues with relevant information
   - Create branches for new PRs
   - Generate and submit pull requests
   - Update issue status/labels based on actions taken

5. **Error Handling & Resilience**
   - Implement comprehensive error boundaries using Effect
   - Log errors with appropriate detail levels
   - Continue operation after non-fatal errors
   - Implement retry mechanisms for transient failures

6. **Configuration Management**
   - Load configuration from environment variables or config file
   - Allow runtime configuration updates
   - Validate configuration values

7. **Execution Loop**
   - Run continuously with configurable intervals
   - Implement graceful shutdown on signals
   - Provide status reports at interval boundaries

## Technical Requirements

1. **Effect Architecture**
   - Use Effect's functional approach for all operations
   - Leverage Effect's tool use functionality for GitHub interactions
   - Implement proper dependency injection patterns

2. **GitHub API Client**
   - Create typed wrappers for GitHub API using Effect's Http client
   - Implement tool requests and TypeSafe schemas for GitHub interactions
   - Implement pagination handling for API responses
   - Ensure proper error handling for API failures (rate limits, not found, etc)
   - Use Effect's Context and Layer patterns for dependency injection

3. **Logging & Monitoring**
   - Structured logging with configurable levels
   - Metrics collection for operation statistics
   - Regular health status reports

4. **Testing Strategy**
   - Comprehensive unit tests for core logic
   - Integration tests for API interactions
   - Mock GitHub API for reliable testing

## Implementation Phases

1. **Phase 1: Core Infrastructure**
   - Set up Effect project structure
   - Implement GitHub API client with authentication
   - Create basic execution loop

2. **Phase 2: Issue Analysis**
   - Build issue fetching and filtering
   - Develop basic analysis logic
   - Implement tracking for processed issues

3. **Phase 3: Action Execution**
   - Add comment posting capability
   - Implement PR creation logic
   - Build response templates

4. **Phase 4: Resilience & Polish**
   - Enhance error handling
   - Add configuration management
   - Implement metrics and reporting

## Tests to Write

1. **GitHub API Client Tests**
   - Authentication flow tests
   - Rate limit handling tests
   - Error response handling tests (file not found, server errors) 
   - Schema validation tests for responses
   - Mock HTTP client tests for GitHub API interactions
   - Tool request schema tests
   - Pagination tests

2. **Issue Processing Tests**
   - Issue fetching and filtering tests
   - Proper handling of various issue formats
   - Issue tracking and deduplication tests

3. **Analysis Engine Tests**
   - Issue content parsing tests
   - Analysis result validation tests
   - Decision logic for different issue types

4. **Action Execution Tests**
   - Comment formatting and posting tests
   - PR creation flow tests
   - Branch management tests
   - Template rendering tests

5. **Configuration Tests**
   - Config loading and validation tests
   - Environment variable handling tests
   - Default configuration tests

6. **Execution Loop Tests**
   - Continuous operation tests
   - Interval timing tests
   - Graceful shutdown tests
   - Recovery from failure tests

7. **Integration Tests**
   - End-to-end workflow tests with mock GitHub API
   - Error recovery integration tests
   - Configuration change handling tests

8. **Error Handling Tests**
   - Various error scenario tests
   - Retry mechanism tests
   - Logging accuracy tests
   - System stability under error conditions