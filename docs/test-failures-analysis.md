# Test Failures Analysis

This document analyzes common test failures in the OpenAgents codebase, focusing on the OIDC signup flow implementation. It serves as a guide for future developers to avoid similar issues.

## Common Failure Patterns

### 1. Tracing Setup Issues

**Problem**: Multiple test failures due to tracing initialization conflicts:
```
Unable to install global subscriber: SetGlobalDefaultError("a global default trace dispatcher has already been set")
```

**Root Cause**: 
- Multiple tests trying to initialize the global tracing subscriber
- Each test file attempting to set up its own tracing configuration

**Solution**:
- Remove tracing setup from individual test files
- Use a single, global tracing setup in test configuration
- Consider using test-specific logging that doesn't require global state

### 2. Mock Response Handling

**Problem**: Tests failing with unexpected error types:
```
Expected AuthenticationFailed, got Err(TokenExchangeFailed("error decoding response body: missing field `id_token` at line 1 column 76"))
```

**Root Cause**:
- Incorrect mock response format
- Error happening at wrong layer (JSON parsing vs JWT validation)
- Mock responses not matching real OIDC server behavior

**Solution**:
- Always use proper JSON structure in mock responses
- Match error types to the actual processing layer:
  ```rust
  // Bad - returns invalid JSON
  .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
  
  // Good - returns valid JSON missing required field
  .respond_with(ResponseTemplate::new(200).set_body_json(json!({
      "access_token": "test_access_token",
      "token_type": "Bearer",
      "expires_in": 3600
  })))
  ```

### 3. Test Isolation Issues

**Problem**: Tests interfering with each other due to shared state:
- Database records from one test affecting another
- Mock server expectations carrying over
- Global state pollution

**Solution**:
- Always clean up test data before and after tests
- Use unique identifiers per test (e.g., test_user_{unique_id})
- Reset mock server between tests
- Avoid global state

### 4. Error Handling Flow

**Problem**: Tests asserting wrong error types because error handling happens in wrong order:
```rust
// Bad - checks JWT before JSON
assert!(matches!(result, Err(AuthError::AuthenticationFailed)));

// Good - JSON parsing error happens first
assert!(matches!(result, Err(AuthError::TokenExchangeFailed(_))));
```

**Solution**:
- Understand and test the error handling flow:
  1. HTTP errors (400, 500, etc.)
  2. JSON parsing errors
  3. Schema validation errors (missing fields)
  4. JWT validation errors
  5. Business logic errors (duplicate users, etc.)

## Best Practices

### 1. Mock Response Structure
```rust
// For testing token exchange failures:
.respond_with(ResponseTemplate::new(400).set_body_json(json!({
    "error": "invalid_request",
    "error_description": "Invalid code"
})))

// For testing JWT validation:
.respond_with(ResponseTemplate::new(200).set_body_json(json!({
    "access_token": "test_access_token",
    "token_type": "Bearer",
    "expires_in": 3600,
    "id_token": "invalid.jwt.format"
})))
```

### 2. Test Database Management
```rust
// In common/mod.rs:
pub async fn setup_test_db() -> PgPool {
    dotenv().ok();
    let pool = PgPool::connect(&database_url).await?;
    
    // Clean up before test
    sqlx::query!("DELETE FROM users WHERE scramble_id LIKE 'test_%'")
        .execute(&pool)
        .await?;
        
    pool
}
```

### 3. Test Organization
```rust
#[tokio::test]
async fn test_signup_error_handling() {
    // Arrange - Set up test environment
    let mock_server = MockServer::start().await;
    let pool = setup_test_db().await;
    let service = create_test_service(&mock_server, pool);

    // Act & Assert - Test each error case separately
    test_malformed_response(&mock_server, &service).await;
    test_invalid_token(&mock_server, &service).await;
    test_invalid_jwt(&mock_server, &service).await;
}
```

## Common Gotchas

1. **Mock Server State**: Each mock response replaces previous ones. Use separate mock servers or reset between tests.

2. **Database Cleanup**: Always clean up test data using patterns that won't affect production data:
   ```sql
   DELETE FROM users WHERE scramble_id LIKE 'test_%'
   ```

3. **Error Order**: Test errors in the order they occur in the code:
   - Network/HTTP errors
   - Response parsing
   - Token validation
   - Business logic

4. **Async Test Timing**: Be aware of async timing issues:
   - Use proper await points
   - Don't rely on timing for test behavior
   - Consider using tokio::time::timeout for potentially hanging tests

## Testing Strategy

1. **Unit Tests**: Test individual components in isolation
   - Mock external services
   - Use in-memory databases when possible
   - Focus on business logic

2. **Integration Tests**: Test complete flows
   - Use test databases
   - Mock external services
   - Test error cases thoroughly

3. **Error Cases**: Test all possible error scenarios
   - Network errors
   - Invalid responses
   - Business logic errors
   - Edge cases

4. **Test Data**: Use clear patterns for test data
   - Prefix test data (e.g., test_*)
   - Use meaningful identifiers
   - Clean up after tests

## Recommendations for New Developers

1. **Read the Tests First**: Understand existing test patterns before adding new ones

2. **Use the Test Utils**: Leverage existing test utilities in common/mod.rs

3. **Follow the Pattern**: Match existing test structure and naming conventions

4. **Clean Up**: Always clean up test data, even if tests fail

5. **Error Handling**: Test error cases thoroughly and in the right order

6. **Mock Responses**: Use realistic mock responses that match actual service behavior

7. **Test Independence**: Ensure each test can run independently

Remember: Tests should be reliable, readable, and maintainable. When tests fail, they should provide clear information about what went wrong and why.