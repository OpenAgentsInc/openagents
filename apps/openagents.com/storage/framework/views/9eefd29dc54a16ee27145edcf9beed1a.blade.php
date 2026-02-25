# Laravel 12

- CRITICAL: ALWAYS use ___SINGLE_BACKTICK___search-docs___SINGLE_BACKTICK___ tool for version-specific Laravel documentation and updated code examples.
@if (file_exists(base_path('app/Http/Kernel.php')))
- This project upgraded from Laravel 10 without migrating to the new streamlined Laravel file structure.
- This is perfectly fine and recommended by Laravel. Follow the existing structure from Laravel 10. We do not need to migrate to the new Laravel structure unless the user explicitly requests it.

## Laravel 10 Structure
- Middleware typically lives in ___SINGLE_BACKTICK___app/Http/Middleware/___SINGLE_BACKTICK___ and service providers in ___SINGLE_BACKTICK___app/Providers/___SINGLE_BACKTICK___.
- There is no ___SINGLE_BACKTICK___bootstrap/app.php___SINGLE_BACKTICK___ application configuration in a Laravel 10 structure:
    - Middleware registration happens in ___SINGLE_BACKTICK___app/Http/Kernel.php___SINGLE_BACKTICK___
    - Exception handling is in ___SINGLE_BACKTICK___app/Exceptions/Handler.php___SINGLE_BACKTICK___
    - Console commands and schedule register in ___SINGLE_BACKTICK___app/Console/Kernel.php___SINGLE_BACKTICK___
    - Rate limits likely exist in ___SINGLE_BACKTICK___RouteServiceProvider___SINGLE_BACKTICK___ or ___SINGLE_BACKTICK___app/Http/Kernel.php___SINGLE_BACKTICK___
@else
- Since Laravel 11, Laravel has a new streamlined file structure which this project uses.

## Laravel 12 Structure
- In Laravel 12, middleware are no longer registered in ___SINGLE_BACKTICK___app/Http/Kernel.php___SINGLE_BACKTICK___.
- Middleware are configured declaratively in ___SINGLE_BACKTICK___bootstrap/app.php___SINGLE_BACKTICK___ using ___SINGLE_BACKTICK___Application::configure()->withMiddleware()___SINGLE_BACKTICK___.
- ___SINGLE_BACKTICK___bootstrap/app.php___SINGLE_BACKTICK___ is the file to register middleware, exceptions, and routing files.
- ___SINGLE_BACKTICK___bootstrap/providers.php___SINGLE_BACKTICK___ contains application specific service providers.
- The ___SINGLE_BACKTICK___app\Console\Kernel.php___SINGLE_BACKTICK___ file no longer exists; use ___SINGLE_BACKTICK___bootstrap/app.php___SINGLE_BACKTICK___ or ___SINGLE_BACKTICK___routes/console.php___SINGLE_BACKTICK___ for console configuration.
- Console commands in ___SINGLE_BACKTICK___app/Console/Commands/___SINGLE_BACKTICK___ are automatically available and do not require manual registration.
@endif

## Database
- When modifying a column, the migration must include all of the attributes that were previously defined on the column. Otherwise, they will be dropped and lost.
- Laravel 12 allows limiting eagerly loaded records natively, without external packages: ___SINGLE_BACKTICK___$query->latest()->limit(10);___SINGLE_BACKTICK___.

### Models
- Casts can and likely should be set in a ___SINGLE_BACKTICK___casts()___SINGLE_BACKTICK___ method on a model rather than the ___SINGLE_BACKTICK___$casts___SINGLE_BACKTICK___ property. Follow existing conventions from other models.
