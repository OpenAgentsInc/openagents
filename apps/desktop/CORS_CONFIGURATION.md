# CORS Configuration for OpenAuth Integration

## Overview

This document outlines the required Cross-Origin Resource Sharing (CORS) configuration for secure OpenAuth JWT authentication integration with the OpenAgents desktop application.

## Phase 4: Production CORS Requirements

### OpenAuth Server Configuration

The OpenAuth server must be configured with proper CORS headers to allow the OpenAgents desktop application to authenticate users securely.

#### Required CORS Headers

```javascript
// Example CORS configuration for OpenAuth server
const corsConfig = {
  // Allow the desktop application to make requests
  origin: [
    'tauri://localhost',           // Tauri development
    'https://tauri.localhost',     // Tauri secure context
    'capacitor://localhost',       // Capacitor context (if applicable)
    'http://localhost:3000',       // Development frontend
    'https://openagents.com',      // Production domain (if applicable)
  ],
  
  // Required methods for OpenAuth flow
  methods: ['GET', 'POST', 'OPTIONS'],
  
  // Required headers for JWT authentication
  allowedHeaders: [
    'Content-Type',
    'Authorization',
    'X-Requested-With',
    'Accept',
    'Origin',
  ],
  
  // Expose headers needed by the client
  exposedHeaders: [
    'Authorization',
    'Content-Type',
  ],
  
  // Allow credentials for secure cookie handling
  credentials: true,
  
  // Cache preflight for 24 hours
  maxAge: 86400,
};
```

#### Cloudflare Workers CORS Implementation

If using Cloudflare Workers for OpenAuth, implement CORS as follows:

```javascript
// cors.js - Cloudflare Workers CORS handler
export function handleCORS(request) {
  const corsHeaders = {
    'Access-Control-Allow-Origin': getTrustedOrigin(request),
    'Access-Control-Allow-Methods': 'GET,HEAD,POST,OPTIONS',
    'Access-Control-Max-Age': '86400',
    'Access-Control-Allow-Headers': 'Content-Type,Authorization,X-Requested-With',
    'Access-Control-Allow-Credentials': 'true',
  };

  // Handle preflight requests
  if (request.method === 'OPTIONS') {
    return new Response(null, {
      status: 200,
      headers: corsHeaders,
    });
  }

  return corsHeaders;
}

function getTrustedOrigin(request) {
  const origin = request.headers.get('Origin');
  const trustedOrigins = [
    'tauri://localhost',
    'https://tauri.localhost',
    'capacitor://localhost',
    'http://localhost:3000',
    'https://openagents.com',
  ];

  return trustedOrigins.includes(origin) ? origin : 'null';
}

// Apply CORS to all OpenAuth endpoints
export default {
  async fetch(request, env, ctx) {
    const corsHeaders = handleCORS(request);
    
    if (request.method === 'OPTIONS') {
      return corsHeaders;
    }

    // Process the actual request
    const response = await handleAuthRequest(request, env);
    
    // Add CORS headers to response
    Object.entries(corsHeaders).forEach(([key, value]) => {
      response.headers.set(key, value);
    });

    return response;
  }
};
```

### Desktop Application Configuration

#### Tauri Configuration

Ensure your `tauri.conf.json` allows the required domains:

```json
{
  "tauri": {
    "allowlist": {
      "http": {
        "all": false,
        "request": true,
        "scope": [
          "https://auth.openagents.com/*",
          "https://*.convex.cloud/*"
        ]
      }
    },
    "security": {
      "csp": "default-src 'self'; connect-src 'self' https://auth.openagents.com https://*.convex.cloud; script-src 'self' 'unsafe-inline';"
    }
  }
}
```

#### HTTP Client Configuration

Configure the Rust HTTP client with proper CORS handling:

```rust
// In convex_impl.rs
use reqwest::Client as HttpClient;

impl EnhancedConvexClient {
    fn create_http_client() -> Result<HttpClient, AppError> {
        HttpClient::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("OpenAgents-Desktop/1.0")
            // Allow cross-origin requests
            .danger_accept_invalid_certs(false)
            .build()
            .map_err(|e| AppError::Http(e))
    }

    async fn make_auth_request(&self, url: &str) -> Result<Response, AppError> {
        let request = self.http_client
            .get(url)
            .header("Accept", "application/json")
            .header("Origin", "tauri://localhost");

        // Add Authorization header if available
        if let Some(auth_header) = self.get_authorization_header().await? {
            request.header("Authorization", auth_header)
        } else {
            request
        }
        .send()
        .await
        .map_err(|e| AppError::Http(e))
    }
}
```

## Security Considerations

### 1. Origin Validation

**Critical**: Always validate the origin of requests to prevent unauthorized access.

```javascript
// Strict origin validation
const ALLOWED_ORIGINS = [
  'tauri://localhost',
  'https://tauri.localhost',
  'https://openagents.com'
];

function validateOrigin(origin) {
  if (!origin || !ALLOWED_ORIGINS.includes(origin)) {
    throw new Error('Unauthorized origin');
  }
  return true;
}
```

### 2. Content Security Policy (CSP)

Implement strict CSP headers on both the OpenAuth server and desktop application:

```
Content-Security-Policy: 
  default-src 'self'; 
  connect-src 'self' https://auth.openagents.com https://*.convex.cloud; 
  script-src 'self' 'unsafe-inline'; 
  style-src 'self' 'unsafe-inline';
  img-src 'self' data: https:;
  font-src 'self';
  object-src 'none';
  base-uri 'self';
  form-action 'self';
  frame-ancestors 'none';
```

### 3. Secure Token Handling

Ensure tokens are handled securely across CORS boundaries:

```rust
// Secure token transmission
impl AuthenticationFlow {
    async fn exchange_code_for_token(&self, code: String) -> Result<TokenResponse, AppError> {
        let request_body = serde_json::json!({
            "grant_type": "authorization_code",
            "code": code,
            "client_id": env::var("OPENAUTH_CLIENT_ID")?,
            // Never include client_secret in frontend code
        });

        let response = self.http_client
            .post("https://auth.openagents.com/token")
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Origin", "tauri://localhost")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AppError::AuthenticationError(
                format!("Token exchange failed: {}", response.status())
            ));
        }

        response.json().await.map_err(|e| AppError::Json(e))
    }
}
```

## Production Deployment Checklist

### OpenAuth Server

- [ ] ✅ CORS headers configured for production domains
- [ ] ✅ Origin validation implemented
- [ ] ✅ HTTPS enforced for all authentication endpoints
- [ ] ✅ CSP headers configured
- [ ] ✅ Rate limiting enabled for authentication endpoints
- [ ] ✅ Security headers (HSTS, X-Frame-Options) configured

### Desktop Application

- [ ] ✅ Tauri CSP configured
- [ ] ✅ HTTP client timeout configured
- [ ] ✅ Secure token storage implemented
- [ ] ✅ Error handling for CORS failures implemented
- [ ] ✅ User-agent string configured
- [ ] ✅ Certificate validation enabled

### DNS and Network

- [ ] ✅ DNS CAA records configured
- [ ] ✅ TLS certificates valid and properly configured
- [ ] ✅ Firewall rules allow necessary traffic
- [ ] ✅ CDN/proxy configuration (if applicable) preserves CORS headers

## Testing CORS Configuration

### Manual Testing

```bash
# Test preflight request
curl -X OPTIONS https://auth.openagents.com/token \\
  -H "Origin: tauri://localhost" \\
  -H "Access-Control-Request-Method: POST" \\
  -H "Access-Control-Request-Headers: Content-Type,Authorization" \\
  -v

# Expected response headers:
# Access-Control-Allow-Origin: tauri://localhost
# Access-Control-Allow-Methods: GET,HEAD,POST,OPTIONS
# Access-Control-Allow-Headers: Content-Type,Authorization,X-Requested-With
# Access-Control-Allow-Credentials: true
```

### Automated Testing

```rust
#[cfg(test)]
mod cors_tests {
    use super::*;

    #[tokio::test]
    async fn test_cors_preflight() {
        let client = HttpClient::new();
        let response = client
            .request(Method::OPTIONS, "https://auth.openagents.com/token")
            .header("Origin", "tauri://localhost")
            .header("Access-Control-Request-Method", "POST")
            .header("Access-Control-Request-Headers", "Content-Type,Authorization")
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers().get("Access-Control-Allow-Origin").unwrap(),
            "tauri://localhost"
        );
    }

    #[tokio::test]
    async fn test_cors_actual_request() {
        let client = HttpClient::new();
        let response = client
            .post("https://auth.openagents.com/token")
            .header("Origin", "tauri://localhost")
            .header("Content-Type", "application/json")
            .json(&json!({
                "grant_type": "authorization_code",
                "code": "test_code"
            }))
            .send()
            .await
            .unwrap();

        // Should include CORS headers even on error responses
        assert!(response.headers().contains_key("Access-Control-Allow-Origin"));
    }
}
```

## Common CORS Issues and Solutions

### Issue 1: "CORS policy: Request header field authorization is not allowed"

**Solution**: Add `Authorization` to `Access-Control-Allow-Headers`:

```javascript
headers['Access-Control-Allow-Headers'] = 'Content-Type,Authorization,X-Requested-With';
```

### Issue 2: "CORS policy: The request client is not a secure context"

**Solution**: Ensure HTTPS is used in production or configure Tauri for secure contexts:

```json
{
  "tauri": {
    "security": {
      "dangerousRemoteDomainIpcAccess": false
    }
  }
}
```

### Issue 3: "CORS policy: Cross origin requests are only supported for protocol schemes: http, data, chrome, chrome-extension, chrome-untrusted, https"

**Solution**: Use standard protocol schemes in development:

```javascript
// Development
const origin = 'http://localhost:3000';

// Production
const origin = 'https://openagents.com';
```

## Monitoring and Logging

### Server-side CORS Monitoring

```javascript
// Log CORS requests for security monitoring
function logCORSRequest(request) {
  const origin = request.headers.get('Origin');
  const userAgent = request.headers.get('User-Agent');
  const method = request.method;
  
  console.log(JSON.stringify({
    timestamp: new Date().toISOString(),
    event: 'cors_request',
    origin,
    userAgent,
    method,
    allowed: ALLOWED_ORIGINS.includes(origin)
  }));
}
```

### Client-side CORS Error Handling

```rust
// Enhanced error handling for CORS issues
impl EnhancedConvexClient {
    async fn handle_cors_error(&self, error: reqwest::Error) -> AppError {
        if error.is_timeout() {
            log::error!("CORS_ERROR: Request timeout - check server CORS configuration");
            AppError::NetworkTimeout("Authentication server timeout".to_string())
        } else if error.is_request() {
            log::error!("CORS_ERROR: Request failed - CORS policy may be blocking request");
            AppError::CorsError("Cross-origin request blocked".to_string())
        } else {
            log::error!("CORS_ERROR: Unexpected error: {}", error);
            AppError::Http(error)
        }
    }
}
```

## Support and Troubleshooting

### Debug Mode

Enable CORS debug logging in development:

```rust
#[cfg(debug_assertions)]
fn log_cors_details(response: &Response) {
    if let Some(origin) = response.headers().get("Access-Control-Allow-Origin") {
        log::debug!("CORS: Allowed origin: {:?}", origin);
    } else {
        log::warn!("CORS: No Access-Control-Allow-Origin header found");
    }
}
```

### Common Error Messages

| Error | Cause | Solution |
|-------|-------|----------|
| `CORS policy: No 'Access-Control-Allow-Origin' header` | Server not sending CORS headers | Configure CORS on OpenAuth server |
| `CORS policy: The request client is not a secure context` | HTTP used instead of HTTPS | Use HTTPS or configure Tauri security |
| `CORS policy: Request header field authorization is not allowed` | Authorization header not in allowed list | Add to Access-Control-Allow-Headers |

### Contact

For CORS configuration issues, contact the OpenAuth team or refer to the Cloudflare Workers CORS documentation.

---

**Security Note**: This configuration is critical for production security. Always test CORS configuration thoroughly before deploying to production.