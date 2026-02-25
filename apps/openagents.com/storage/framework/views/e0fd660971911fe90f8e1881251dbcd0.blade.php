@php
/** @var \Laravel\Boost\Install\GuidelineAssist $assist */
@endphp
# Do Things the Laravel Way

- Use ___SINGLE_BACKTICK___{{ $assist->artisanCommand('make:') }}___SINGLE_BACKTICK___ commands to create new files (i.e. migrations, controllers, models, etc.). You can list available Artisan commands using the ___SINGLE_BACKTICK___list-artisan-commands___SINGLE_BACKTICK___ tool.
- If you're creating a generic PHP class, use ___SINGLE_BACKTICK___{{ $assist->artisanCommand('make:class') }}___SINGLE_BACKTICK___.
- Pass ___SINGLE_BACKTICK___--no-interaction___SINGLE_BACKTICK___ to all Artisan commands to ensure they work without user input. You should also pass the correct ___SINGLE_BACKTICK___--options___SINGLE_BACKTICK___ to ensure correct behavior.

## Database
- Always use proper Eloquent relationship methods with return type hints. Prefer relationship methods over raw queries or manual joins.
- Use Eloquent models and relationships before suggesting raw database queries.
- Avoid ___SINGLE_BACKTICK___DB::___SINGLE_BACKTICK___; prefer ___SINGLE_BACKTICK___Model::query()___SINGLE_BACKTICK___. Generate code that leverages Laravel's ORM capabilities rather than bypassing them.
- Generate code that prevents N+1 query problems by using eager loading.
- Use Laravel's query builder for very complex database operations.

### Model Creation
- When creating new models, create useful factories and seeders for them too. Ask the user if they need any other things, using ___SINGLE_BACKTICK___list-artisan-commands___SINGLE_BACKTICK___ to check the available options to ___SINGLE_BACKTICK___{{ $assist->artisanCommand('make:model') }}___SINGLE_BACKTICK___.

### APIs & Eloquent Resources
- For APIs, default to using Eloquent API Resources and API versioning unless existing API routes do not, then you should follow existing application convention.

## Controllers & Validation
- Always create Form Request classes for validation rather than inline validation in controllers. Include both validation rules and custom error messages.
- Check sibling Form Requests to see if the application uses array or string based validation rules.

## Authentication & Authorization
- Use Laravel's built-in authentication and authorization features (gates, policies, Sanctum, etc.).

## URL Generation
- When generating links to other pages, prefer named routes and the ___SINGLE_BACKTICK___route()___SINGLE_BACKTICK___ function.

## Queues
- Use queued jobs for time-consuming operations with the ___SINGLE_BACKTICK___ShouldQueue___SINGLE_BACKTICK___ interface.

## Configuration
- Use environment variables only in configuration files - never use the ___SINGLE_BACKTICK___env()___SINGLE_BACKTICK___ function directly outside of config files. Always use ___SINGLE_BACKTICK___config('app.name')___SINGLE_BACKTICK___, not ___SINGLE_BACKTICK___env('APP_NAME')___SINGLE_BACKTICK___.

## Testing
- When creating models for tests, use the factories for the models. Check if the factory has custom states that can be used before manually setting up the model.
- Faker: Use methods such as ___SINGLE_BACKTICK___$this->faker->word()___SINGLE_BACKTICK___ or ___SINGLE_BACKTICK___fake()->randomDigit()___SINGLE_BACKTICK___. Follow existing conventions whether to use ___SINGLE_BACKTICK___$this->faker___SINGLE_BACKTICK___ or ___SINGLE_BACKTICK___fake()___SINGLE_BACKTICK___.
- When creating tests, make use of ___SINGLE_BACKTICK___{{ $assist->artisanCommand('make:test [options] {name}') }}___SINGLE_BACKTICK___ to create a feature test, and pass ___SINGLE_BACKTICK___--unit___SINGLE_BACKTICK___ to create a unit test. Most tests should be feature tests.

## Vite Error
- If you receive an "Illuminate\Foundation\ViteException: Unable to locate file in Vite manifest" error, you can run ___SINGLE_BACKTICK___{{ $assist->nodePackageManagerCommand('run build') }}___SINGLE_BACKTICK___ or ask the user to run ___SINGLE_BACKTICK___{{ $assist->nodePackageManagerCommand('run dev') }}___SINGLE_BACKTICK___ or ___SINGLE_BACKTICK___{{ $assist->composerCommand('run dev') }}___SINGLE_BACKTICK___.
