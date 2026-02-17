# OpenAPI (Auto-Generated)

This app uses `vyuldashev/laravel-openapi` to generate the API specification from controller attributes.

## URL

- Production/OpenAPI URL: `https://openagents.com/openapi.json`
- Local URL (when app is running): `http://localhost:8000/openapi.json`

## How Generation Works

The spec is generated in two ways:

1. Runtime route (from package config):
   - `config/openapi.php` exposes the route at `/openapi.json`.
2. Deploy/build-time generation (required gate):
   - `Dockerfile` runs `php artisan openapi:generate --output=public/openapi.json` during image build.
   - Build fails if this command fails.

This means deploys will not succeed unless a valid spec can be generated.

## Local Regeneration

From `apps/openagents.com/`:

```bash
php artisan openapi:generate --output=public/openapi.json
```

## Source of Truth

- Endpoint operations live on controllers via attributes:
  - `#[OpenApi\PathItem]`
  - `#[OpenApi\Operation]`
  - `#[OpenApi\Response]`
  - `#[OpenApi\RequestBody]`
  - `#[OpenApi\Parameters]`
- OpenAPI component factories live under:
  - `app/OpenApi/Responses`
  - `app/OpenApi/RequestBodies`
  - `app/OpenApi/Parameters`
  - `app/OpenApi/SecuritySchemes`

## Security

Global API security in `config/openapi.php` is configured as bearer token:

- Security scheme: `SanctumToken`
- Type: HTTP bearer
- Intended use: `Authorization: Bearer <sanctum-personal-access-token>`
