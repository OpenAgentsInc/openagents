# Configuration Guide

This document explains how OpenAgents handles configuration, environment variables, and database connections.

## Configuration Overview

OpenAgents uses a layered configuration system that combines:
- YAML configuration files
- Environment variables
- Platform-specific settings (e.g., DigitalOcean App Platform)

## Configuration Files

Configuration files are located in the `configuration/` directory:

```
configuration/
├── base.yaml      # Base configuration, used in all environments
├── local.yaml     # Local development overrides
└── production.yaml # Production environment overrides
```

### Environment Selection

The active environment is determined by the `APP_ENVIRONMENT` variable:
- Defaults to `local` if not set
- Valid values: `local`, `production`

## Database Configuration

### Priority Order

1. `DATABASE_URL` environment variable (highest priority)
2. Environment-specific configuration file (e.g., `production.yaml`)
3. Base configuration (`base.yaml`)

### Using DATABASE_URL

When deploying to platforms like DigitalOcean, the `DATABASE_URL` environment variable is the preferred method:

```
DATABASE_URL=postgres://username:password@host:port/database
```

When `DATABASE_URL` is present:
- SSL is automatically enabled (required for DigitalOcean)
- Connection timeout is set to 10 seconds
- Statement cache is initially disabled
- Slow query logging is enabled (1 second threshold)

### Configuration File Settings

Database settings in YAML files:

```yaml
database:
  host: "127.0.0.1"
  port: 5432
  username: "postgres"
  password: "password"
  database_name: "openagents"
  require_ssl: false
  max_connection_retries: 5
  retry_interval_secs: 5
```

### SSL Configuration

- When using `DATABASE_URL`: SSL is always required
- When using configuration files: SSL mode is determined by `require_ssl`
  - `true`: Forces SSL connection
  - `false`: Prefers SSL but allows non-SSL

### Connection Parameters

Default connection settings:
- Connection timeout: 10 seconds
- Statement cache: Initially disabled
- Application name: "openagents"
- Slow query logging threshold: 1 second

## Environment Variables

### Required Variables

- `APP_ENVIRONMENT`: Deployment environment (`local` or `production`)
- `PORT`: Server port (automatically set by some platforms)
- `DATABASE_URL`: Database connection string (recommended for production)

### Optional Variables

- `APP_APPLICATION__PORT`: Override application port
- `APP_APPLICATION__HOST`: Override application host
- `APP_DATABASE__*`: Override individual database settings

### Variable Naming Convention

Environment variables can override any configuration setting using the format:
`APP_SECTION__KEY`

Examples:
- `APP_APPLICATION__PORT=8080`
- `APP_DATABASE__HOST=custom-host`

## Logging and Diagnostics

The application logs detailed configuration information at startup:

1. Environment Detection:
   - Current environment
   - Configuration file paths
   - Present environment variables (excluding sensitive data)

2. Database Configuration:
   - Connection source (DATABASE_URL or configuration files)
   - Host and port
   - SSL mode
   - Connection parameters

3. Connection Attempts:
   - Parse results for DATABASE_URL
   - Connection timeouts
   - SSL/TLS status
   - Retry attempts

## Platform-Specific Considerations

### DigitalOcean App Platform

When deploying to DigitalOcean:
1. Use the provided `DATABASE_URL`
2. SSL is automatically required
3. The `PORT` environment variable is automatically set
4. Bind to `0.0.0.0` for proper routing

Example DigitalOcean environment setup:
```
APP_ENVIRONMENT=production
DATABASE_URL=(provided by DigitalOcean)
PORT=(provided by DigitalOcean)
```

## Troubleshooting

Common issues and solutions:

1. Database Connection Failures:
   - Verify DATABASE_URL format
   - Check SSL requirements
   - Verify network access and firewall rules
   - Review application logs for connection details

2. Configuration Issues:
   - Verify environment variable names and format
   - Check configuration file syntax
   - Ensure proper environment selection

3. SSL/TLS Issues:
   - Verify SSL is enabled for production
   - Check certificate validity
   - Ensure proper SSL mode configuration

## Best Practices

1. Production Deployments:
   - Always use `DATABASE_URL` when provided
   - Set `APP_ENVIRONMENT=production`
   - Enable SSL for database connections
   - Use secrets management for sensitive values

2. Local Development:
   - Use `local.yaml` for development settings
   - Keep `base.yaml` as a template
   - Don't commit sensitive credentials

3. Configuration Management:
   - Use environment variables for sensitive data
   - Keep configuration files in version control
   - Document all custom environment variables

## Monitoring and Debugging

The application provides detailed logs for configuration and connection issues:

```rust
// Example log output
info!("Starting configuration loading process");
info!("Environment: production");
info!("Database configuration source: DATABASE_URL");
info!("Database connection details:");
info!("  Host: db.example.com");
info!("  Port: 5432");
info!("  SSL Mode: Required");
```

For additional debugging, check application logs for:
- Configuration loading process
- Database connection attempts
- SSL/TLS negotiation
- Connection timeouts or failures