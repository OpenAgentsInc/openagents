import { Effect, Data, Schedule, Schema } from "effect";

/**
 * HttpClientService - Comprehensive HTTP client with Effect patterns
 * 
 * Following EffectPatterns best practices:
 * - Retry patterns with exponential backoff
 * - Timeout handling for all requests
 * - Schema validation for type-safe responses
 * - Tagged errors for precise error handling
 * - Dependency injection ready
 * - Testable with mock implementations
 */

// Tagged error types for HTTP operations
export class HttpError extends Data.TaggedError("HttpError")<{
  method: string;
  url: string;
  status?: number;
  message: string;
  cause?: unknown;
}> {}

export class TimeoutError extends Data.TaggedError("TimeoutError")<{
  method: string;
  url: string;
  timeoutMs: number;
}> {}

export class ParseError extends Data.TaggedError("ParseError")<{
  method: string;
  url: string;
  contentType?: string;
  message: string;
  cause?: unknown;
}> {}

export class ValidationError extends Data.TaggedError("ValidationError")<{
  method: string;
  url: string;
  schema: string;
  errors: unknown[];
}> {}

export class NetworkError extends Data.TaggedError("NetworkError")<{
  method: string;
  url: string;
  message: string;
  cause?: unknown;
}> {}

// HTTP configuration interface
export interface HttpConfig {
  baseURL?: string;
  timeout: number;
  retries: number;
  retryDelayMs: number;
  headers?: Record<string, string>;
}

// Request options interface
export interface RequestOptions {
  headers?: Record<string, string>;
  timeout?: number;
  retries?: number;
  validateStatus?: (status: number) => boolean;
}

// Default HTTP configuration
const DEFAULT_CONFIG: HttpConfig = {
  timeout: 30000,
  retries: 3,
  retryDelayMs: 100,
  headers: {
    "Content-Type": "application/json",
    "Accept": "application/json"
  }
};

/**
 * HttpClientService using Effect.Service pattern for dependency injection
 * 
 * This follows the "handle-flaky-operations-with-retry-timeout" pattern from EffectPatterns,
 * providing robust HTTP communication with automatic retry and timeout handling.
 */
export class HttpClientService extends Effect.Service<HttpClientService>()(
  "HttpClientService",
  {
    sync: (config: HttpConfig = DEFAULT_CONFIG) => {
      // Merge provided config with defaults
      const mergedConfig = { ...DEFAULT_CONFIG, ...config };

      // Helper to build full URL
      const buildUrl = (path: string): string => {
        if (path.startsWith("http://") || path.startsWith("https://")) {
          return path;
        }
        return mergedConfig.baseURL ? `${mergedConfig.baseURL}${path}` : path;
      };

      // Helper to merge headers
      const mergeHeaders = (requestHeaders?: Record<string, string>): Record<string, string> => ({
        ...mergedConfig.headers,
        ...requestHeaders
      });

      // Core HTTP request function with retry and timeout
      const makeRequest = <T>(
        method: string,
        url: string,
        options: RequestOptions & { body?: unknown } = {}
      ) => Effect.gen(function* () {
        const fullUrl = buildUrl(url);
        const headers = mergeHeaders(options.headers);
        const timeout = options.timeout ?? mergedConfig.timeout;
        const retries = options.retries ?? mergedConfig.retries;
        
        // Create the base request effect
        const requestEffect = Effect.tryPromise({
          try: async () => {
            const requestInit: RequestInit = {
              method,
              headers,
              body: options.body ? JSON.stringify(options.body) : undefined
            };

            const response = await fetch(fullUrl, requestInit);
            
            // Check if status should be considered an error
            const isValidStatus = options.validateStatus 
              ? options.validateStatus(response.status)
              : response.status >= 200 && response.status < 300;

            if (!isValidStatus) {
              throw new HttpError({
                method,
                url: fullUrl,
                status: response.status,
                message: `HTTP ${response.status}: ${response.statusText}`
              });
            }

            return response;
          },
          catch: (error) => {
            // Network errors (no response received)
            if (error instanceof TypeError && error.message.includes("fetch")) {
              return new NetworkError({
                method,
                url: fullUrl,
                message: error.message,
                cause: error
              });
            }
            
            // Re-throw our custom errors
            if (error instanceof HttpError) {
              return error;
            }

            // Generic HTTP error
            return new HttpError({
              method,
              url: fullUrl,
              message: String(error),
              cause: error
            });
          }
        });

        // Apply timeout wrapper
        const withTimeout = requestEffect.pipe(
          Effect.timeout(`${timeout} millis`),
          Effect.catchTag("TimeoutException", () => 
            Effect.fail(new TimeoutError({
              method,
              url: fullUrl,
              timeoutMs: timeout
            }))
          )
        );

        // Apply retry logic with exponential backoff
        const withRetry = withTimeout.pipe(
          Effect.retry(
            Schedule.exponential(`${mergedConfig.retryDelayMs} millis`).pipe(
              Schedule.intersect(Schedule.recurs(retries))
            )
          )
        );

        return yield* withRetry;
      });

      return {
        /**
         * GET request with optional schema validation
         */
        get: <T>(
          url: string, 
          schema?: Schema.Schema<T, unknown>,
          options: RequestOptions = {}
        ) => Effect.gen(function* () {
          const response = yield* makeRequest("GET", url, options);
          
          if (!schema) {
            // Return raw response if no schema provided - parse JSON first
            const data = yield* Effect.tryPromise({
              try: () => response.json(),
              catch: (error) => new ParseError({
                method: "GET",
                url: buildUrl(url),
                contentType: response.headers.get("content-type") || "",
                message: "Failed to parse JSON response",
                cause: error
              })
            });
            return data as T;
          }

          // Parse and validate response
          const contentType = response.headers.get("content-type") || "";
          
          try {
            const data = yield* Effect.tryPromise({
              try: () => response.json(),
              catch: (error) => new ParseError({
                method: "GET",
                url: buildUrl(url),
                contentType,
                message: "Failed to parse JSON response",
                cause: error
              })
            });

            return yield* Schema.decode(schema)(data).pipe(
              Effect.catchTag("ParseError", (error) =>
                Effect.fail(new ValidationError({
                  method: "GET",
                  url: buildUrl(url),
                  schema: schema.toString(),
                  errors: [String(error)]
                }))
              )
            );
          } catch (error) {
            return yield* Effect.fail(new ParseError({
              method: "GET",
              url: buildUrl(url),
              contentType,
              message: String(error),
              cause: error
            }));
          }
        }),

        /**
         * POST request with optional schema validation
         */
        post: <T>(
          url: string,
          body: unknown,
          schema?: Schema.Schema<T, unknown>,
          options: RequestOptions = {}
        ) => Effect.gen(function* () {
          const response = yield* makeRequest("POST", url, { ...options, body });
          
          if (!schema) {
            return response as T;
          }

          const contentType = response.headers.get("content-type") || "";
          
          try {
            const data = yield* Effect.tryPromise({
              try: () => response.json(),
              catch: (error) => new ParseError({
                method: "POST",
                url: buildUrl(url),
                contentType,
                message: "Failed to parse JSON response",
                cause: error
              })
            });

            return yield* Schema.decode(schema)(data).pipe(
              Effect.catchTag("ParseError", (error) =>
                Effect.fail(new ValidationError({
                  method: "POST",
                  url: buildUrl(url),
                  schema: schema.toString(),
                  errors: [String(error)]
                }))
              )
            );
          } catch (error) {
            return yield* Effect.fail(new ParseError({
              method: "POST",
              url: buildUrl(url),
              contentType,
              message: String(error),
              cause: error
            }));
          }
        }),

        /**
         * PUT request with optional schema validation
         */
        put: <T>(
          url: string,
          body: unknown,
          schema?: Schema.Schema<T, unknown>,
          options: RequestOptions = {}
        ) => Effect.gen(function* () {
          const response = yield* makeRequest("PUT", url, { ...options, body });
          
          if (!schema) {
            return response as T;
          }

          const contentType = response.headers.get("content-type") || "";
          
          try {
            const data = yield* Effect.tryPromise({
              try: () => response.json(),
              catch: (error) => new ParseError({
                method: "PUT",
                url: buildUrl(url),
                contentType,
                message: "Failed to parse JSON response",
                cause: error
              })
            });

            return yield* Schema.decode(schema)(data).pipe(
              Effect.catchTag("ParseError", (error) =>
                Effect.fail(new ValidationError({
                  method: "PUT",
                  url: buildUrl(url),
                  schema: schema.toString(),
                  errors: [String(error)]
                }))
              )
            );
          } catch (error) {
            return yield* Effect.fail(new ParseError({
              method: "PUT",
              url: buildUrl(url),
              contentType,
              message: String(error),
              cause: error
            }));
          }
        }),

        /**
         * DELETE request
         */
        delete: (url: string, options: RequestOptions = {}) => 
          makeRequest("DELETE", url, options).pipe(
            Effect.map(() => void 0)
          ),

        /**
         * Generic request method for custom HTTP methods
         */
        request: <T>(
          method: string,
          url: string,
          options: RequestOptions & { body?: unknown } = {}
        ) => makeRequest<T>(method, url, options),

        /**
         * Health check endpoint for service monitoring
         */
        healthCheck: (url: string = "/health") => Effect.gen(function* () {
          const start = Date.now();
          
          try {
            yield* makeRequest("GET", url, { timeout: 5000, retries: 0 });
            const duration = Date.now() - start;
            
            return {
              status: "healthy" as const,
              responseTime: duration,
              timestamp: new Date().toISOString()
            };
          } catch (error) {
            const duration = Date.now() - start;
            
            return {
              status: "unhealthy" as const,
              responseTime: duration,
              timestamp: new Date().toISOString(),
              error: String(error)
            };
          }
        }),

        /**
         * Get current configuration
         */
        getConfig: () => Effect.succeed({ ...mergedConfig }),

        /**
         * Create a new instance with updated configuration
         */
        withConfig: (newConfig: Partial<HttpConfig>): HttpClientService => {
          const updatedConfig = { ...mergedConfig, ...newConfig };
          return HttpClientService.of(HttpClientService.sync(updatedConfig));
        }
      };
    }
  }
) {
  /**
   * Test implementation for mocking in tests
   */
  static Test = (mockResponses: Map<string, unknown> = new Map()) =>
    HttpClientService.of({
      _tag: "HttpClientService" as const,
      get: (url, schema) => {
        const mockResponse = mockResponses.get(`GET:${url}`);
        if (mockResponse === undefined) {
          return Effect.fail(new HttpError({
            method: "GET",
            url,
            status: 404,
            message: "Mock response not found"
          }));
        }
        
        if (schema) {
          return Schema.decode(schema)(mockResponse).pipe(
            Effect.catchTag("ParseError", (error) =>
              Effect.fail(new ValidationError({
                method: "GET",
                url,
                schema: schema.toString(),
                errors: [String(error)]
              }))
            )
          );
        }
        
        return Effect.succeed(mockResponse);
      },
      
      post: (url, body, schema) => {
        const mockResponse = mockResponses.get(`POST:${url}`);
        if (mockResponse === undefined) {
          return Effect.fail(new HttpError({
            method: "POST",
            url,
            status: 404,
            message: "Mock response not found"
          }));
        }
        
        if (schema) {
          return Schema.decode(schema)(mockResponse).pipe(
            Effect.catchTag("ParseError", (error) =>
              Effect.fail(new ValidationError({
                method: "POST",
                url,
                schema: schema.toString(),
                errors: [String(error)]
              }))
            )
          );
        }
        
        return Effect.succeed(mockResponse);
      },

      put: (url, body, schema) => {
        const mockResponse = mockResponses.get(`PUT:${url}`);
        if (mockResponse === undefined) {
          return Effect.fail(new HttpError({
            method: "PUT",
            url,
            status: 404,
            message: "Mock response not found"
          }));
        }
        
        if (schema) {
          return Schema.decode(schema)(mockResponse).pipe(
            Effect.catchTag("ParseError", (error) =>
              Effect.fail(new ValidationError({
                method: "PUT",
                url,
                schema: schema.toString(),
                errors: [String(error)]
              }))
            )
          );
        }
        
        return Effect.succeed(mockResponse);
      },

      delete: () => Effect.succeed(void 0),
      
      request: (method, url) => {
        const mockResponse = mockResponses.get(`${method}:${url}`);
        return mockResponse !== undefined 
          ? Effect.succeed(mockResponse)
          : Effect.fail(new HttpError({
              method,
              url,
              status: 404,
              message: "Mock response not found"
            }));
      },

      healthCheck: () => Effect.succeed({
        status: "healthy" as const,
        responseTime: 1,
        timestamp: new Date().toISOString()
      }),

      getConfig: () => Effect.succeed(DEFAULT_CONFIG),
      
      withConfig: (newConfig): HttpClientService => HttpClientService.Test(mockResponses)
    });
}

// Note: HttpClientService.Default is automatically created by Effect.Service pattern