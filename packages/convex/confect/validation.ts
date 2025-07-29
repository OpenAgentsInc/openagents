import { Effect, Data, Schema } from "effect";

// Validation error types
export class ValidationError extends Data.TaggedError("ValidationError")<{
  field: string;
  value: unknown;
  message: string;
  rule: string;
}> {}

export class SanitizationError extends Data.TaggedError("SanitizationError")<{
  field: string;
  originalValue: unknown;
  message: string;
}> {}

// Input sanitization functions
export const sanitizeString = (input: string, maxLength: number = 1000): Effect.Effect<string, SanitizationError, never> =>
  Effect.gen(function* () {
    if (typeof input !== 'string') {
      return yield* Effect.fail(new SanitizationError({
        field: 'string',
        originalValue: input,
        message: 'Input must be a string'
      }));
    }
    
    // Remove potentially dangerous characters
    let sanitized = input
      .replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, '') // Remove script tags
      .replace(/on\w+\s*=/gi, '') // Remove event handlers
      .trim();
    
    // More comprehensive javascript protocol removal from href attributes
    sanitized = sanitized.replace(/href\s*=\s*["']javascript:[^"']*["']/gi, 'href=""');
    sanitized = sanitized.replace(/javascript:/gi, ''); // General javascript: protocol removal
    
    // Enforce length limits
    const truncated = sanitized.length > maxLength 
      ? sanitized.substring(0, maxLength) 
      : sanitized;
    
    return truncated;
  });

export const sanitizeEmail = (email: string): Effect.Effect<string, ValidationError, never> =>
  Effect.gen(function* () {
    const sanitized = yield* sanitizeString(email, 254); // RFC 5321 limit
    
    // Basic email validation
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (!emailRegex.test(sanitized)) {
      return yield* Effect.fail(new ValidationError({
        field: 'email',
        value: sanitized,
        message: 'Invalid email format',
        rule: 'email_format'
      }));
    }
    
    return sanitized.toLowerCase();
  });

export const sanitizeGitHubUsername = (username: string): Effect.Effect<string, ValidationError, never> =>
  Effect.gen(function* () {
    // Check original length before sanitization
    if (username.length > 39) {
      return yield* Effect.fail(new ValidationError({
        field: 'githubUsername',
        value: username,
        message: 'Invalid GitHub username format',
        rule: 'github_username_format'
      }));
    }

    const sanitized = yield* sanitizeString(username, 39); // GitHub username limit
    
    // GitHub username validation - more strict rules
    // Cannot start/end with hyphen, no consecutive hyphens, alphanumeric + hyphens only
    const usernameRegex = /^[a-zA-Z0-9]([a-zA-Z0-9]|-(?!-))*[a-zA-Z0-9]$|^[a-zA-Z0-9]$/;
    
    if (!usernameRegex.test(sanitized) || 
        sanitized.length === 0 || 
        sanitized.startsWith('-') || 
        sanitized.endsWith('-') || 
        sanitized.includes('--') ||
        sanitized.includes(' ')) {
      return yield* Effect.fail(new ValidationError({
        field: 'githubUsername',
        value: sanitized,
        message: 'Invalid GitHub username format',
        rule: 'github_username_format'
      }));
    }
    
    return sanitized;
  });

export const sanitizeProjectPath = (path: string): Effect.Effect<string, ValidationError, never> =>
  Effect.gen(function* () {
    const sanitized = yield* sanitizeString(path, 500);
    
    // Basic path validation - no null bytes, no ../ sequences
    if (sanitized.includes('\0') || sanitized.includes('../')) {
      return yield* Effect.fail(new ValidationError({
        field: 'projectPath',
        value: sanitized,
        message: 'Invalid characters in project path',
        rule: 'safe_path'
      }));
    }
    
    return sanitized;
  });

export const sanitizeSessionId = (sessionId: string): Effect.Effect<string, ValidationError, never> =>
  Effect.gen(function* () {
    const sanitized = yield* sanitizeString(sessionId, 100);
    
    // Session ID should be alphanumeric with hyphens
    const sessionIdRegex = /^[a-zA-Z0-9\-_]+$/;
    if (!sessionIdRegex.test(sanitized)) {
      return yield* Effect.fail(new ValidationError({
        field: 'sessionId',
        value: sanitized,
        message: 'Session ID contains invalid characters',
        rule: 'alphanumeric_session_id'
      }));
    }
    
    return sanitized;
  });

export const sanitizeMessageContent = (content: string): Effect.Effect<string, ValidationError, never> =>
  Effect.gen(function* () {
    const sanitized = yield* sanitizeString(content, 10000); // 10KB limit for messages
    
    // Ensure content is not empty after sanitization
    if (sanitized.length === 0) {
      return yield* Effect.fail(new ValidationError({
        field: 'content',
        value: content,
        message: 'Message content cannot be empty after sanitization',
        rule: 'non_empty_content'
      }));
    }
    
    return sanitized;
  });

// Enhanced validation schemas with sanitization
export const SafeUserInput = Schema.Struct({
  email: Schema.String.pipe(
    Schema.transform(
      Schema.String,
      s => sanitizeEmail(s),
      s => Effect.succeed(s)
    )
  ),
  githubUsername: Schema.String.pipe(
    Schema.transform(
      Schema.String,
      s => sanitizeGitHubUsername(s),
      s => Effect.succeed(s)
    )
  ),
  name: Schema.optional(
    Schema.String.pipe(
      Schema.transform(
        Schema.String,
        s => sanitizeString(s, 100),
        s => Effect.succeed(s)
      )
    )
  ),
});

export const SafeSessionInput = Schema.Struct({
  sessionId: Schema.String.pipe(
    Schema.transform(
      Schema.String,
      s => sanitizeSessionId(s),
      s => Effect.succeed(s)
    )
  ),
  projectPath: Schema.String.pipe(
    Schema.transform(
      Schema.String,
      s => sanitizeProjectPath(s),
      s => Effect.succeed(s)
    )
  ),
  title: Schema.optional(
    Schema.String.pipe(
      Schema.transform(
        Schema.String,
        s => sanitizeString(s, 200),
        s => Effect.succeed(s)
      )
    )
  ),
});

export const SafeMessageInput = Schema.Struct({
  content: Schema.String.pipe(
    Schema.transform(
      Schema.String,
      s => sanitizeMessageContent(s),
      s => Effect.succeed(s)
    )
  ),
  sessionId: Schema.String.pipe(
    Schema.transform(
      Schema.String,
      s => sanitizeSessionId(s),
      s => Effect.succeed(s)
    )
  ),
});

// Rate limiting utilities
export const createRateLimiter = (maxRequests: number, windowMs: number) => {
  const requests = new Map<string, number[]>();
  
  return (identifier: string): Effect.Effect<boolean, never, never> =>
    Effect.sync(() => {
      const now = Date.now();
      const userRequests = requests.get(identifier) || [];
      
      // Remove old requests outside the window
      const recentRequests = userRequests.filter(time => now - time < windowMs);
      
      if (recentRequests.length >= maxRequests) {
        return false; // Rate limit exceeded
      }
      
      // Add current request
      recentRequests.push(now);
      requests.set(identifier, recentRequests);
      
      return true; // Request allowed
    });
};

// Default rate limiters
export const userActionRateLimit = createRateLimiter(100, 60000); // 100 requests per minute
export const authActionRateLimit = createRateLimiter(10, 60000); // 10 auth requests per minute