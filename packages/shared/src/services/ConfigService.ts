import { Effect, Data, Schema, Layer, Context } from "effect";

/**
 * ConfigService - Configuration management with Schema validation
 * 
 * Following EffectPatterns best practices:
 * - Schema-first configuration definition
 * - Environment-aware configuration loading
 * - Compile-time type safety with runtime validation
 * - Tagged errors for configuration issues
 * - Layer-based dependency injection
 * - Hot reloading capability for development
 */

// Tagged error types for configuration operations
export class ConfigError extends Data.TaggedError("ConfigError")<{
  operation: string;
  key?: string;
  message: string;
  cause?: unknown;
}> {}

export class ConfigValidationError extends Data.TaggedError("ConfigValidationError")<{
  key: string;
  value: unknown;
  errors: unknown[];
}> {}

export class ConfigMissingError extends Data.TaggedError("ConfigMissingError")<{
  key: string;
  environment: string;
}> {}

// Environment type
export type Environment = "development" | "production" | "test" | "staging";

// Core application configuration schema
export const AppConfigSchema = Schema.Struct({
  // Application settings
  app: Schema.Struct({
    name: Schema.String,
    version: Schema.String,
    environment: Schema.Literal("development", "production", "test", "staging"),
    debug: Schema.Boolean,
    logLevel: Schema.Literal("error", "warn", "info", "debug", "trace")
  }),

  // API configuration
  api: Schema.Struct({
    baseUrl: Schema.String,
    timeout: Schema.Number.pipe(Schema.positive()),
    retries: Schema.Number.pipe(Schema.int(), Schema.between(0, 10)),
    rateLimit: Schema.Struct({
      requests: Schema.Number.pipe(Schema.positive()),
      windowMs: Schema.Number.pipe(Schema.positive())
    })
  }),

  // Database configuration
  database: Schema.Struct({
    url: Schema.String,
    maxConnections: Schema.Number.pipe(Schema.positive()),
    timeout: Schema.Number.pipe(Schema.positive()),
    ssl: Schema.Boolean
  }),

  // Authentication configuration
  auth: Schema.Struct({
    jwtSecret: Schema.String.pipe(Schema.minLength(32)),
    tokenExpiry: Schema.String,
    refreshTokenExpiry: Schema.String,
    oauth: Schema.Struct({
      providers: Schema.Array(Schema.Literal("google", "github", "apple")),
      redirectUrl: Schema.String
    })
  }),

  // Feature flags
  features: Schema.Struct({
    enableAnalytics: Schema.Boolean,
    enableNotifications: Schema.Boolean,
    enableExperimentalFeatures: Schema.Boolean,
    maxFileSize: Schema.Number.pipe(Schema.positive()),
    maxConcurrentSessions: Schema.Number.pipe(Schema.positive())
  }),

  // Storage configuration
  storage: Schema.Struct({
    provider: Schema.Literal("local", "s3", "gcs"),
    bucket: Schema.optional(Schema.String),
    region: Schema.optional(Schema.String),
    accessKey: Schema.optional(Schema.String),
    secretKey: Schema.optional(Schema.String)
  })
});

export type AppConfig = Schema.Schema.Type<typeof AppConfigSchema>;

// Development-specific configuration schema
export const DevConfigSchema = Schema.Struct({
  hotReload: Schema.Boolean,
  mockServices: Schema.Boolean,
  debugTools: Schema.Boolean,
  verbose: Schema.Boolean
});

export type DevConfig = Schema.Schema.Type<typeof DevConfigSchema>;

// Combined configuration schema
export const FullConfigSchema = Schema.Struct({
  ...AppConfigSchema.fields,
  dev: Schema.optional(DevConfigSchema)
});

export type FullConfig = Schema.Schema.Type<typeof FullConfigSchema>;

// Default configurations for different environments
const createDefaultConfig = (env: Environment): AppConfig => ({
  app: {
    name: "OpenAgents",
    version: "0.0.3",
    environment: env,
    debug: env === "development",
    logLevel: env === "production" ? "warn" : "debug"
  },
  api: {
    baseUrl: env === "production" ? "https://api.openagents.com" : "http://localhost:3000",
    timeout: 30000,
    retries: env === "production" ? 3 : 1,
    rateLimit: {
      requests: 100,
      windowMs: 60000
    }
  },
  database: {
    url: env === "test" ? "sqlite://memory" : "postgresql://localhost:5432/openagents",
    maxConnections: env === "production" ? 20 : 5,
    timeout: 5000,
    ssl: env === "production"
  },
  auth: {
    jwtSecret: env === "production" 
      ? "REPLACE_WITH_SECURE_SECRET_IN_PRODUCTION_32_CHARS_MIN" 
      : "development_jwt_secret_key_32_chars_minimum",
    tokenExpiry: "1h",
    refreshTokenExpiry: "7d",
    oauth: {
      providers: env === "production" ? ["google", "github", "apple"] : ["google"],
      redirectUrl: env === "production" 
        ? "https://openagents.com/auth/callback" 
        : "http://localhost:3000/auth/callback"
    }
  },
  features: {
    enableAnalytics: env === "production",
    enableNotifications: true,
    enableExperimentalFeatures: env === "development",
    maxFileSize: 50 * 1024 * 1024, // 50MB
    maxConcurrentSessions: env === "production" ? 10 : 3
  },
  storage: {
    provider: env === "production" ? "s3" : "local",
    bucket: env === "production" ? "openagents-prod" : undefined,
    region: env === "production" ? "us-east-1" : undefined,
    accessKey: undefined,
    secretKey: undefined
  }
});

/**
 * ConfigService using Effect.Service pattern for dependency injection
 * 
 * This follows the "define-config-schema" and "provide-config-layer" patterns 
 * from EffectPatterns, providing type-safe configuration management.
 */
export class ConfigService extends Effect.Service<ConfigService>()(
  "ConfigService",
  {
    sync: (initialConfig?: Partial<FullConfig>) => {
      // Internal configuration state
      let currentConfig: FullConfig;
      
      // Environment detection
      const detectEnvironment = (): Environment => {
        if (typeof process !== "undefined" && process.env.NODE_ENV) {
          const env = process.env.NODE_ENV as Environment;
          if (["development", "production", "test", "staging"].includes(env)) {
            return env;
          }
        }
        return "development";
      };

      // Load configuration from environment variables (returns empty for overrides)
      const loadFromEnvironment = (): Partial<FullConfig> => {
        // For now, keep environment loading simple to avoid schema conflicts
        // Environment variables can override defaults in a future iteration
        return {};
      };

      // Initialize configuration
      const initializeConfig = (): Effect.Effect<FullConfig, ConfigValidationError> => Effect.gen(function* () {
        const environment = detectEnvironment();
        const defaults = createDefaultConfig(environment);
        const envConfig = loadFromEnvironment();
        
        // Deep merge configurations: defaults < env < initial
        const mergedConfig = {
          ...defaults,
          ...envConfig,
          ...initialConfig,
          // Ensure dev config exists for development
          dev: environment === "development" ? {
            hotReload: true,
            mockServices: false,
            debugTools: true,
            verbose: false,
            ...envConfig.dev,
            ...initialConfig?.dev
          } : envConfig.dev || initialConfig?.dev
        };

        // Validate the final configuration
        const validatedConfig = yield* Schema.decode(FullConfigSchema)(mergedConfig).pipe(
          Effect.catchTag("ParseError", (error) =>
            Effect.fail(new ConfigValidationError({
              key: "root",
              value: mergedConfig,
              errors: [String(error)]
            }))
          )
        );

        return validatedConfig;
      });

      // Initialize the configuration on service creation (using a default for now)
      const environment = detectEnvironment();
      currentConfig = {
        ...createDefaultConfig(environment),
        dev: environment === "development" ? {
          hotReload: true,
          mockServices: false,
          debugTools: true,
          verbose: false
        } : undefined
      };

      return {
        /**
         * Get the complete configuration
         */
        getConfig: () => Effect.succeed(currentConfig),

        /**
         * Get a specific configuration value by path
         */
        get: <K extends keyof FullConfig>(key: K): Effect.Effect<FullConfig[K], ConfigMissingError> => Effect.gen(function* () {
          const value = currentConfig[key];
          if (value === undefined) {
            yield* Effect.fail(new ConfigMissingError({
              key: key as string,
              environment: currentConfig.app.environment
            }));
          }
          return value;
        }),

        /**
         * Get a nested configuration value
         */
        getPath: <T>(path: string, defaultValue?: T): Effect.Effect<T, ConfigMissingError> => Effect.gen(function* () {
          const keys = path.split(".");
          let value: any = currentConfig;
          
          for (const key of keys) {
            if (value && typeof value === "object" && key in value) {
              value = value[key];
            } else {
              if (defaultValue !== undefined) {
                return defaultValue;
              }
              yield* Effect.fail(new ConfigMissingError({
                key: path,
                environment: currentConfig.app.environment
              }));
            }
          }
          
          return value as T;
        }),

        /**
         * Check if we're in a specific environment
         */
        isEnvironment: (env: Environment) => 
          Effect.succeed(currentConfig.app.environment === env),

        /**
         * Check if a feature is enabled
         */
        isFeatureEnabled: (feature: keyof AppConfig["features"]) =>
          Effect.succeed(currentConfig.features[feature]),

        /**
         * Get environment-specific configuration
         */
        getEnvironmentConfig: () => Effect.gen(function* () {
          const env = currentConfig.app.environment;
          
          return {
            environment: env,
            isDevelopment: env === "development",
            isProduction: env === "production",
            isTest: env === "test",
            isStaging: env === "staging",
            debug: currentConfig.app.debug,
            logLevel: currentConfig.app.logLevel
          };
        }),

        /**
         * Validate a configuration object against the schema
         */
        validate: <T>(data: unknown, schema: Schema.Schema<T>): Effect.Effect<T, ConfigValidationError> => 
          Schema.decode(schema)(data as T).pipe(
            Effect.catchTag("ParseError", (error) =>
              Effect.fail(new ConfigValidationError({
                key: "validation",
                value: data,
                errors: [String(error)]
              }))
            )
          ),

        /**
         * Hot reload configuration (development only)
         */
        reload: () => Effect.gen(function* () {
          if (currentConfig.app.environment !== "development") {
            yield* Effect.fail(new ConfigError({
              operation: "reload",
              message: "Hot reload is only available in development environment"
            }));
          }

          try {
            const newConfig = yield* initializeConfig();
            currentConfig = newConfig;
            return newConfig;
          } catch (error) {
            yield* Effect.fail(new ConfigError({
              operation: "reload", 
              message: "Failed to reload configuration",
              cause: error
            }));
          }
        }),

        /**
         * Update configuration at runtime (with validation)
         */
        update: (updates: Partial<FullConfig>) => Effect.gen(function* () {
          const updatedConfig = { ...currentConfig, ...updates };
          
          const validatedConfig = yield* Schema.decode(FullConfigSchema)(updatedConfig).pipe(
            Effect.catchTag("ParseError", (error) =>
              Effect.fail(new ConfigValidationError({
                key: "update",
                value: updatedConfig,
                errors: [String(error)]
              }))
            )
          );

          currentConfig = validatedConfig;
          return validatedConfig;
        }),

        /**
         * Get configuration for a specific service
         */
        getServiceConfig: <T>(serviceName: string, schema: Schema.Schema<T>): Effect.Effect<T, ConfigValidationError> => Effect.gen(function* () {
          const serviceConfig = (currentConfig as any).services?.[serviceName] || {};

          return yield* Schema.decode(schema)(serviceConfig).pipe(
            Effect.catchTag("ParseError", (error) =>
              Effect.fail(new ConfigValidationError({
                key: `services.${serviceName}`,
                value: serviceConfig,
                errors: [String(error)]
              }))
            )
          );
        })
      };
    }
  }
) {
  /**
   * Test implementation with mock configuration
   */
  static Test = (testConfig: Partial<FullConfig> = {}) => {
    const mockConfig: FullConfig = {
      ...createDefaultConfig("test"),
      ...testConfig,
      dev: {
        hotReload: false,
        mockServices: true,
        debugTools: true,
        verbose: false,
        ...testConfig.dev
      }
    };

    return ConfigService.of({
      _tag: "ConfigService" as const,
      getConfig: () => Effect.succeed(mockConfig),
      get: (key) => Effect.succeed(mockConfig[key]),
      getPath: (path, defaultValue) => {
        const keys = path.split(".");
        let value: any = mockConfig;
        for (const key of keys) {
          if (value && typeof value === "object" && key in value) {
            value = value[key];
          } else {
            return defaultValue !== undefined 
              ? Effect.succeed(defaultValue)
              : Effect.fail(new ConfigMissingError({ key: path, environment: "test" }));
          }
        }
        return Effect.succeed(value);
      },
      isEnvironment: (env) => Effect.succeed(env === "test"),
      isFeatureEnabled: (feature) => Effect.succeed(mockConfig.features[feature]),
      getEnvironmentConfig: () => Effect.succeed({
        environment: "test" as Environment,
        isDevelopment: false,
        isProduction: false,
        isTest: true,
        isStaging: false,
        debug: true,
        logLevel: "debug" as const
      }),
      validate: <T>(data: unknown, schema: Schema.Schema<T>) => Schema.decode(schema)(data as T).pipe(
        Effect.catchTag("ParseError", (error) =>
          Effect.fail(new ConfigValidationError({
            key: "test-validation",
            value: data,
            errors: [String(error)]
          }))
        )
      ),
      reload: () => Effect.succeed(mockConfig),
      update: (updates) => Effect.succeed({ ...mockConfig, ...updates }),
      getServiceConfig: <T>(serviceName: string, schema: Schema.Schema<T>) => Schema.decode(schema)({} as T).pipe(
        Effect.catchTag("ParseError", (error) =>
          Effect.fail(new ConfigValidationError({
            key: `test-services.${serviceName}`,
            value: {},
            errors: [String(error)]
          }))
        )
      )
    });
  };

  /**
   * Layer that provides the test ConfigService
   */
  static TestLive = (testConfig?: Partial<FullConfig>) => 
    Layer.succeed(ConfigService, ConfigService.Test(testConfig));
}

// Note: ConfigService.Default is automatically created by Effect.Service pattern