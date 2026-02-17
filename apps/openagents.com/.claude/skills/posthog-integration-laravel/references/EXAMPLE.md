# PostHog Laravel Example Project

Repository: https://github.com/PostHog/context-mill
Path: basics/laravel

---

## README.md

# PostHog Laravel Example

A Laravel application demonstrating PostHog integration for analytics, feature flags, and error tracking using Livewire for reactive UI components.

## Features

- User registration and authentication with Livewire
- SQLite database persistence with Eloquent ORM
- User identification and property tracking
- Custom event tracking (burrito consideration tracker)
- Page view tracking (dashboard, profile)
- Feature flags with payload support
- Error tracking with manual exception capture
- Reactive UI components with Livewire

## Tech Stack

- **Framework**: Laravel 11.x
- **Reactive Components**: Livewire 3.x
- **Database**: SQLite
- **Analytics**: PostHog PHP SDK

## Quick Start

**Note**: This is a minimal implementation demonstrating PostHog integration. For a production application, you would need to install Laravel via Composer and set up additional dependencies.

### Manual Setup (Demonstration)

1. Install dependencies:
   ```bash
   composer install
   ```

2. Set up environment:
   ```bash
   cp .env.example .env
   # Edit .env with your PostHog API key
   ```

3. Configure PostHog in `.env`:
   ```env
   POSTHOG_API_KEY=your_posthog_api_key
   POSTHOG_HOST=https://us.i.posthog.com
   POSTHOG_DISABLED=false
   ```

4. Generate application key:
   ```bash
   php artisan key:generate
   ```

5. Create database and run migrations:
   ```bash
   touch database/database.sqlite
   php artisan migrate --seed
   ```

6. Start the development server:
   ```bash
   php artisan serve
   ```

7. Open http://localhost:8000 and either:
   - Login with default credentials: `admin@example.com` / `admin`
   - Or click "Sign up here" to create a new account

## PostHog Service

The `PostHogService` class (`app/Services/PostHogService.php`) wraps the PostHog PHP SDK and provides:

| Method | Description |
|--------|-------------|
| `identify($distinctId, $properties)` | Identify a user with properties |
| `capture($distinctId, $event, $properties)` | Capture custom events |
| `captureException($exception, $distinctId)` | Capture exceptions with stack traces |
| `isFeatureEnabled($key, $distinctId, $properties)` | Check feature flag status |
| `getFeatureFlagPayload($key, $distinctId)` | Get feature flag payload |

All methods check `config('posthog.disabled')` and return early if PostHog is disabled.

## PostHog Integration Points

### User Registration (`app/Http/Livewire/Auth/Register.php`)
New users are identified and tracked on signup:
```php
$posthog->identify($user->email, $user->getPostHogProperties());
$posthog->capture($user->email, 'user_signed_up', [
    'signup_method' => 'form',
]);
```

### User Login (`app/Http/Livewire/Auth/Login.php`)
Users are identified on login with their properties:
```php
$posthog->identify($user->email, $user->getPostHogProperties());
$posthog->capture($user->email, 'user_logged_in', [
    'login_method' => 'password',
]);
```

### User Logout (`routes/web.php`)
Logout events are tracked:
```php
$posthog->capture($user->email, 'user_logged_out');
```

### Page View Tracking
Dashboard and profile views are tracked (`app/Http/Livewire/Dashboard.php`, `app/Http/Livewire/Profile.php`):
```php
$posthog->capture($user->email, 'dashboard_viewed', [
    'is_staff' => $user->is_staff,
]);

$posthog->capture($user->email, 'profile_viewed');
```

### Custom Event Tracking (`app/Http/Livewire/BurritoTracker.php`)
The burrito tracker demonstrates custom event capture:
```php
$posthog->identify($user->email, $user->getPostHogProperties());
$posthog->capture($user->email, 'burrito_considered', [
    'total_considerations' => $this->burritoCount,
]);
```

### Feature Flags (`app/Http/Livewire/Dashboard.php`)
The dashboard demonstrates feature flag checking:
```php
$this->showNewFeature = $posthog->isFeatureEnabled(
    'new-dashboard-feature',
    $user->email,
    $user->getPostHogProperties()
) ?? false;

$this->featureConfig = $posthog->getFeatureFlagPayload(
    'new-dashboard-feature',
    $user->email
);
```

### Error Tracking
Manual exception capture is demonstrated in multiple places:

**Livewire Components** (`app/Http/Livewire/Dashboard.php`, `app/Http/Livewire/Profile.php`):
```php
try {
    throw new \Exception('This is a test error for PostHog tracking');
} catch (\Exception $e) {
    $errorId = $posthog->captureException($e, $user->email);
    $this->successMessage = "Error captured in PostHog! Error ID: {$errorId}";
}
```

**API Endpoint** (`app/Http/Controllers/Api/ErrorTestController.php`):
```php
try {
    throw new \Exception('Test exception from critical operation');
} catch (\Throwable $e) {
    if ($shouldCapture) {
        $posthog->identify($user->email, $user->getPostHogProperties());
        $eventId = $posthog->captureException($e, $user->email);

        return response()->json([
            'error' => 'Operation failed',
            'error_id' => $eventId,
            'message' => "Error captured in PostHog. Reference ID: {$eventId}",
        ], 500);
    }
}
```

The `/api/test-error` endpoint demonstrates manual exception capture. Use `?capture=true` to capture in PostHog, or `?capture=false` to skip tracking.


## Pages

| Route | Component | PostHog Events |
|-------|-----------|----------------|
| `/` | Login | `user_logged_in` |
| `/register` | Register | `user_signed_up` |
| `/dashboard` | Dashboard | `dashboard_viewed`, feature flag checks |
| `/burrito` | BurritoTracker | `burrito_considered` |
| `/profile` | Profile | `profile_viewed` |
| `/logout` | (route) | `user_logged_out` |

## Project Structure

```
basics/laravel/
├── app/
│   ├── Http/
│   │   ├── Controllers/
│   │   │   └── Api/
│   │   │       ├── BurritoController.php   # Burrito API endpoint
│   │   │       └── ErrorTestController.php # Error testing endpoint
│   │   └── Livewire/
│   │       ├── Auth/
│   │       │   ├── Login.php               # Login component
│   │       │   └── Register.php            # Registration component
│   │       ├── BurritoTracker.php          # Burrito tracker component
│   │       ├── Dashboard.php               # Dashboard with feature flags
│   │       └── Profile.php                 # User profile component
│   ├── Models/
│   │   └── User.php                        # User model with PostHog properties
│   └── Services/
│       └── PostHogService.php              # PostHog wrapper service
├── database/
│   ├── migrations/                         # Database migrations
│   └── seeders/
│       └── DatabaseSeeder.php              # Seeds admin user
├── resources/
│   └── views/
│       ├── components/
│       │   └── layouts/
│       │       ├── app.blade.php           # Authenticated layout
│       │       └── guest.blade.php         # Guest layout
│       ├── errors/
│       │   ├── 404.blade.php               # Not found page
│       │   └── 500.blade.php               # Server error page
│       └── livewire/
│           ├── auth/
│           │   ├── login.blade.php         # Login form
│           │   └── register.blade.php      # Registration form
│           ├── burrito-tracker.blade.php   # Burrito tracker UI
│           ├── dashboard.blade.php         # Dashboard UI
│           └── profile.blade.php           # Profile UI
├── routes/
│   ├── web.php                             # Web routes (auth, pages)
│   └── api.php                             # API routes
└── config/
    └── posthog.php                         # PostHog configuration
```

## Development Commands

```bash
# Start development server
php artisan serve

# Run migrations
php artisan migrate

# Seed database
php artisan migrate:fresh --seed

# Clear caches
php artisan optimize:clear
```
---

## .env.example

```example
APP_NAME="PostHog Laravel Example"
APP_ENV=local
APP_KEY=
APP_DEBUG=true
APP_URL=http://localhost:8000

DB_CONNECTION=sqlite
# DB_DATABASE will use default database/database.sqlite

CACHE_DRIVER=file
CACHE_STORE=file

SESSION_DRIVER=file
SESSION_LIFETIME=120

POSTHOG_API_KEY=your_posthog_api_key_here
POSTHOG_HOST=https://us.i.posthog.com
POSTHOG_DISABLED=false

```

---

## app/Http/Controllers/Api/BurritoController.php

```php
<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Services\PostHogService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;

class BurritoController extends Controller
{
    public function consider(Request $request, PostHogService $posthog): JsonResponse
    {
        $user = Auth::user();

        // Increment session counter
        $burritoCount = session('burrito_count', 0) + 1;
        session(['burrito_count' => $burritoCount]);

        // PostHog: Track event
        $posthog->identify($user->email, $user->getPostHogProperties());
        $posthog->capture($user->email, 'burrito_considered', [
            'total_considerations' => $burritoCount,
        ]);

        return response()->json([
            'success' => true,
            'count' => $burritoCount,
        ]);
    }
}

```

---

## app/Http/Controllers/Api/ErrorTestController.php

```php
<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Services\PostHogService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;

class ErrorTestController extends Controller
{
    public function test(Request $request, PostHogService $posthog): JsonResponse
    {
        $shouldCapture = $request->query('capture', 'true') === 'true';
        $user = Auth::user();

        try {
            throw new \Exception('Test exception from critical operation');
        } catch (\Throwable $e) {
            if ($shouldCapture) {
                // Capture in PostHog
                $posthog->identify($user->email, $user->getPostHogProperties());
                $eventId = $posthog->captureException($e, $user->email);

                return response()->json([
                    'error' => 'Operation failed',
                    'error_id' => $eventId,
                    'message' => "Error captured in PostHog. Reference ID: {$eventId}",
                ], 500);
            }

            return response()->json([
                'error' => $e->getMessage(),
            ], 500);
        }
    }
}

```

---

## app/Http/Controllers/Controller.php

```php
<?php

namespace App\Http\Controllers;

abstract class Controller
{
    //
}

```

---

## app/Http/Livewire/Auth/Login.php

```php
<?php

namespace App\Http\Livewire\Auth;

use App\Services\PostHogService;
use Illuminate\Support\Facades\Auth;
use Livewire\Component;

class Login extends Component
{
    public string $email = '';
    public string $password = '';
    public bool $remember = false;

    protected $rules = [
        'email' => 'required|email',
        'password' => 'required',
    ];

    public function login(PostHogService $posthog)
    {
        $this->validate();

        if (Auth::attempt(['email' => $this->email, 'password' => $this->password], $this->remember)) {
            $user = Auth::user();

            // PostHog: Identify and track login
            $posthog->identify($user->email, $user->getPostHogProperties());
            $posthog->capture($user->email, 'user_logged_in', [
                'login_method' => 'password',
            ]);

            session()->regenerate();

            return redirect()->intended(route('dashboard'));
        }

        $this->addError('email', 'Invalid credentials');
    }

    public function render()
    {
        return view('livewire.auth.login')
            ->layout('components.layouts.guest');
    }
}

```

---

## app/Http/Livewire/Auth/Register.php

```php
<?php

namespace App\Http\Livewire\Auth;

use App\Models\User;
use App\Services\PostHogService;
use Illuminate\Support\Facades\Auth;
use Livewire\Component;

class Register extends Component
{
    public string $email = '';
    public string $password = '';
    public string $password_confirmation = '';

    protected $rules = [
        'email' => 'required|email|unique:users,email',
        'password' => 'required|min:6|confirmed',
    ];

    public function register(PostHogService $posthog)
    {
        $validated = $this->validate();

        $user = User::create([
            'email' => $validated['email'],
            'password' => bcrypt($validated['password']),
            'is_staff' => false,
        ]);

        // PostHog: Identify new user and track signup
        $posthog->identify($user->email, $user->getPostHogProperties());
        $posthog->capture($user->email, 'user_signed_up', [
            'signup_method' => 'form',
        ]);

        Auth::login($user);

        session()->flash('success', 'Account created successfully!');

        return redirect()->route('dashboard');
    }

    public function render()
    {
        return view('livewire.auth.register')
            ->layout('components.layouts.guest');
    }
}

```

---

## app/Http/Livewire/BurritoTracker.php

```php
<?php

namespace App\Http\Livewire;

use App\Services\PostHogService;
use Illuminate\Support\Facades\Auth;
use Livewire\Component;

class BurritoTracker extends Component
{
    public int $burritoCount = 0;

    public function mount()
    {
        $this->burritoCount = session('burrito_count', 0);
    }

    public function considerBurrito(PostHogService $posthog)
    {
        $this->burritoCount++;
        session(['burrito_count' => $this->burritoCount]);

        // PostHog: Track burrito consideration
        $user = Auth::user();
        $posthog->identify($user->email, $user->getPostHogProperties());
        $posthog->capture($user->email, 'burrito_considered', [
            'total_considerations' => $this->burritoCount,
        ]);

        $this->dispatch('burrito-considered');
    }

    public function render()
    {
        return view('livewire.burrito-tracker')
            ->layout('components.layouts.app');
    }
}

```

---

## app/Http/Livewire/Dashboard.php

```php
<?php

namespace App\Http\Livewire;

use App\Services\PostHogService;
use Illuminate\Support\Facades\Auth;
use Livewire\Component;

class Dashboard extends Component
{
    public bool $showNewFeature = false;
    public $featureConfig = null;
    public ?string $errorMessage = null;
    public ?string $successMessage = null;

    public function mount(PostHogService $posthog)
    {
        $user = Auth::user();

        // PostHog: Track dashboard view
        $posthog->capture($user->email, 'dashboard_viewed', [
            'is_staff' => $user->is_staff,
        ]);

        // Check feature flag
        $this->showNewFeature = $posthog->isFeatureEnabled(
            'new-dashboard-feature',
            $user->email,
            $user->getPostHogProperties()
        ) ?? false;

        // Get feature flag payload
        $this->featureConfig = $posthog->getFeatureFlagPayload(
            'new-dashboard-feature',
            $user->email
        );
    }

    public function testErrorWithCapture(PostHogService $posthog)
    {
        $user = Auth::user();

        try {
            // Simulate an error
            throw new \Exception('This is a test error for PostHog tracking');
        } catch (\Exception $e) {
            // Capture the exception in PostHog
            $errorId = $posthog->captureException($e, $user->email);

            $this->successMessage = "Error captured in PostHog! Error ID: {$errorId}";
            $this->errorMessage = null;
        }
    }

    public function testErrorWithoutCapture()
    {
        try {
            // Simulate an error without capturing
            throw new \Exception('This error was NOT sent to PostHog');
        } catch (\Exception $e) {
            $this->errorMessage = "Error occurred but NOT captured in PostHog: " . $e->getMessage();
            $this->successMessage = null;
        }
    }

    public function render()
    {
        return view('livewire.dashboard')
            ->layout('components.layouts.app');
    }
}

```

---

## app/Http/Livewire/Profile.php

```php
<?php

namespace App\Http\Livewire;

use App\Services\PostHogService;
use Illuminate\Support\Facades\Auth;
use Livewire\Component;

class Profile extends Component
{
    public ?string $errorMessage = null;
    public ?string $successMessage = null;

    public function mount(PostHogService $posthog)
    {
        $user = Auth::user();

        // PostHog: Track profile view
        $posthog->capture($user->email, 'profile_viewed');
    }

    public function testErrorWithCapture(PostHogService $posthog)
    {
        $user = Auth::user();

        try {
            // Simulate an error
            throw new \Exception('This is a test error for PostHog tracking');
        } catch (\Exception $e) {
            // Capture the exception in PostHog
            $errorId = $posthog->captureException($e, $user->email);

            $this->successMessage = "Error captured in PostHog! Error ID: {$errorId}";
            $this->errorMessage = null;
        }
    }

    public function testErrorWithoutCapture()
    {
        try {
            // Simulate an error without capturing
            throw new \Exception('This error was NOT sent to PostHog');
        } catch (\Exception $e) {
            $this->errorMessage = "Error occurred but NOT captured in PostHog: " . $e->getMessage();
            $this->successMessage = null;
        }
    }

    public function render()
    {
        return view('livewire.profile')
            ->layout('components.layouts.app');
    }
}

```

---

## app/Models/User.php

```php
<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Foundation\Auth\User as Authenticatable;
use Illuminate\Notifications\Notifiable;

class User extends Authenticatable
{
    use HasFactory, Notifiable;

    protected $fillable = [
        'email',
        'password',
        'is_staff',
    ];

    protected $hidden = [
        'password',
        'remember_token',
    ];

    protected function casts(): array
    {
        return [
            'password' => 'hashed',
            'is_staff' => 'boolean',
        ];
    }

    /**
     * Get PostHog person properties for this user.
     */
    public function getPostHogProperties(): array
    {
        return [
            'email' => $this->email,
            'is_staff' => $this->is_staff,
            'date_joined' => $this->created_at->toISOString(),
        ];
    }
}

```

---

## app/Services/PostHogService.php

```php
<?php

namespace App\Services;

use PostHog\PostHog;
use Illuminate\Support\Facades\Auth;

class PostHogService
{
    protected static $initialized = false;

    public function __construct()
    {
        if (config('posthog.disabled')) {
            return;
        }

        // Initialize PostHog once
        if (!self::$initialized) {
            PostHog::init(
                config('posthog.api_key'),
                [
                    'host' => config('posthog.host'),
                    'debug' => config('posthog.debug'),
                ]
            );
            self::$initialized = true;
        }
    }

    public function identify(string $distinctId, array $properties = []): void
    {
        if (config('posthog.disabled')) {
            return;
        }

        PostHog::identify([
            'distinctId' => $distinctId,
            'properties' => $properties,
        ]);
    }

    public function capture(string $distinctId, string $event, array $properties = []): void
    {
        if (config('posthog.disabled')) {
            return;
        }

        PostHog::capture([
            'distinctId' => $distinctId,
            'event' => $event,
            'properties' => $properties,
        ]);
    }

    public function captureException(\Throwable $exception, ?string $distinctId = null): ?string
    {
        if (config('posthog.disabled')) {
            return null;
        }

        $distinctId = $distinctId ?? Auth::user()?->email ?? 'anonymous';

        $eventId = uniqid('error_', true);

        $this->capture($distinctId, '$exception', [
            'error_id' => $eventId,
            'exception_type' => get_class($exception),
            'exception_message' => $exception->getMessage(),
            'exception_file' => $exception->getFile(),
            'exception_line' => $exception->getLine(),
            'stack_trace' => $exception->getTraceAsString(),
        ]);

        return $eventId;
    }

    public function isFeatureEnabled(string $key, string $distinctId, array $properties = []): ?bool
    {
        if (config('posthog.disabled')) {
            return false;
        }

        return PostHog::isFeatureEnabled($key, $distinctId, $properties);
    }

    public function getFeatureFlagPayload(string $key, string $distinctId)
    {
        if (config('posthog.disabled')) {
            return null;
        }

        return PostHog::getFeatureFlagPayload($key, $distinctId);
    }
}

```

---

## artisan

```
#!/usr/bin/env php
<?php

define('LARAVEL_START', microtime(true));

/*
|--------------------------------------------------------------------------
| Register The Auto Loader
|--------------------------------------------------------------------------
*/

require __DIR__.'/vendor/autoload.php';

$app = require_once __DIR__.'/bootstrap/app.php';

/*
|--------------------------------------------------------------------------
| Run The Artisan Application
|--------------------------------------------------------------------------
*/

$kernel = $app->make(Illuminate\Contracts\Console\Kernel::class);

$status = $kernel->handle(
    $input = new Symfony\Component\Console\Input\ArgvInput,
    new Symfony\Component\Console\Output\ConsoleOutput
);

$kernel->terminate($input, $status);

exit($status);

```

---

## bootstrap/app.php

```php
<?php

use Illuminate\Foundation\Application;
use Illuminate\Foundation\Configuration\Exceptions;
use Illuminate\Foundation\Configuration\Middleware;

return Application::configure(basePath: dirname(__DIR__))
    ->withRouting(
        web: __DIR__.'/../routes/web.php',
        api: __DIR__.'/../routes/api.php',
        commands: __DIR__.'/../routes/console.php',
        health: '/up',
    )
    ->withMiddleware(function (Middleware $middleware) {
        //
    })
    ->withExceptions(function (Exceptions $exceptions) {
        //
    })->create();

```

---

## bootstrap/cache/packages.php

```php
<?php return array (
  'livewire/livewire' => 
  array (
    'aliases' => 
    array (
      'Livewire' => 'Livewire\\Livewire',
    ),
    'providers' => 
    array (
      0 => 'Livewire\\LivewireServiceProvider',
    ),
  ),
  'nesbot/carbon' => 
  array (
    'providers' => 
    array (
      0 => 'Carbon\\Laravel\\ServiceProvider',
    ),
  ),
  'nunomaduro/termwind' => 
  array (
    'providers' => 
    array (
      0 => 'Termwind\\Laravel\\TermwindServiceProvider',
    ),
  ),
);
```

---

## bootstrap/cache/services.php

```php
<?php return array (
  'providers' => 
  array (
    0 => 'Illuminate\\Auth\\AuthServiceProvider',
    1 => 'Illuminate\\Broadcasting\\BroadcastServiceProvider',
    2 => 'Illuminate\\Bus\\BusServiceProvider',
    3 => 'Illuminate\\Cache\\CacheServiceProvider',
    4 => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    5 => 'Illuminate\\Cookie\\CookieServiceProvider',
    6 => 'Illuminate\\Database\\DatabaseServiceProvider',
    7 => 'Illuminate\\Encryption\\EncryptionServiceProvider',
    8 => 'Illuminate\\Filesystem\\FilesystemServiceProvider',
    9 => 'Illuminate\\Foundation\\Providers\\FoundationServiceProvider',
    10 => 'Illuminate\\Hashing\\HashServiceProvider',
    11 => 'Illuminate\\Mail\\MailServiceProvider',
    12 => 'Illuminate\\Notifications\\NotificationServiceProvider',
    13 => 'Illuminate\\Pagination\\PaginationServiceProvider',
    14 => 'Illuminate\\Pipeline\\PipelineServiceProvider',
    15 => 'Illuminate\\Queue\\QueueServiceProvider',
    16 => 'Illuminate\\Redis\\RedisServiceProvider',
    17 => 'Illuminate\\Auth\\Passwords\\PasswordResetServiceProvider',
    18 => 'Illuminate\\Session\\SessionServiceProvider',
    19 => 'Illuminate\\Translation\\TranslationServiceProvider',
    20 => 'Illuminate\\Validation\\ValidationServiceProvider',
    21 => 'Illuminate\\View\\ViewServiceProvider',
    22 => 'Livewire\\LivewireServiceProvider',
    23 => 'Carbon\\Laravel\\ServiceProvider',
    24 => 'Termwind\\Laravel\\TermwindServiceProvider',
  ),
  'eager' => 
  array (
    0 => 'Illuminate\\Auth\\AuthServiceProvider',
    1 => 'Illuminate\\Cookie\\CookieServiceProvider',
    2 => 'Illuminate\\Database\\DatabaseServiceProvider',
    3 => 'Illuminate\\Encryption\\EncryptionServiceProvider',
    4 => 'Illuminate\\Filesystem\\FilesystemServiceProvider',
    5 => 'Illuminate\\Foundation\\Providers\\FoundationServiceProvider',
    6 => 'Illuminate\\Notifications\\NotificationServiceProvider',
    7 => 'Illuminate\\Pagination\\PaginationServiceProvider',
    8 => 'Illuminate\\Session\\SessionServiceProvider',
    9 => 'Illuminate\\View\\ViewServiceProvider',
    10 => 'Livewire\\LivewireServiceProvider',
    11 => 'Carbon\\Laravel\\ServiceProvider',
    12 => 'Termwind\\Laravel\\TermwindServiceProvider',
  ),
  'deferred' => 
  array (
    'Illuminate\\Broadcasting\\BroadcastManager' => 'Illuminate\\Broadcasting\\BroadcastServiceProvider',
    'Illuminate\\Contracts\\Broadcasting\\Factory' => 'Illuminate\\Broadcasting\\BroadcastServiceProvider',
    'Illuminate\\Contracts\\Broadcasting\\Broadcaster' => 'Illuminate\\Broadcasting\\BroadcastServiceProvider',
    'Illuminate\\Bus\\Dispatcher' => 'Illuminate\\Bus\\BusServiceProvider',
    'Illuminate\\Contracts\\Bus\\Dispatcher' => 'Illuminate\\Bus\\BusServiceProvider',
    'Illuminate\\Contracts\\Bus\\QueueingDispatcher' => 'Illuminate\\Bus\\BusServiceProvider',
    'Illuminate\\Bus\\BatchRepository' => 'Illuminate\\Bus\\BusServiceProvider',
    'Illuminate\\Bus\\DatabaseBatchRepository' => 'Illuminate\\Bus\\BusServiceProvider',
    'cache' => 'Illuminate\\Cache\\CacheServiceProvider',
    'cache.store' => 'Illuminate\\Cache\\CacheServiceProvider',
    'cache.psr6' => 'Illuminate\\Cache\\CacheServiceProvider',
    'memcached.connector' => 'Illuminate\\Cache\\CacheServiceProvider',
    'Illuminate\\Cache\\RateLimiter' => 'Illuminate\\Cache\\CacheServiceProvider',
    'Illuminate\\Foundation\\Console\\AboutCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Cache\\Console\\ClearCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Cache\\Console\\ForgetCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ClearCompiledCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Auth\\Console\\ClearResetsCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ConfigCacheCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ConfigClearCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ConfigShowCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\DbCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\MonitorCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\PruneCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\ShowCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\TableCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\WipeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\DownCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\EnvironmentCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\EnvironmentDecryptCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\EnvironmentEncryptCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\EventCacheCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\EventClearCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\EventListCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Concurrency\\Console\\InvokeSerializedClosureCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\KeyGenerateCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\OptimizeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\OptimizeClearCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\PackageDiscoverCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Cache\\Console\\PruneStaleTagsCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\ClearCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\ListFailedCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\FlushFailedCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\ForgetFailedCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\ListenCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\MonitorCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\PruneBatchesCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\PruneFailedJobsCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\RestartCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\RetryCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\RetryBatchCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\WorkCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\RouteCacheCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\RouteClearCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\RouteListCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\DumpCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\Seeds\\SeedCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Console\\Scheduling\\ScheduleFinishCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Console\\Scheduling\\ScheduleListCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Console\\Scheduling\\ScheduleRunCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Console\\Scheduling\\ScheduleClearCacheCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Console\\Scheduling\\ScheduleTestCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Console\\Scheduling\\ScheduleWorkCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Console\\Scheduling\\ScheduleInterruptCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\ShowModelCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\StorageLinkCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\StorageUnlinkCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\UpCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ViewCacheCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ViewClearCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ApiInstallCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\BroadcastingInstallCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Cache\\Console\\CacheTableCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\CastMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ChannelListCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ChannelMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ClassMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ComponentMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ConfigPublishCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ConsoleMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Routing\\Console\\ControllerMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\DocsCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\EnumMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\EventGenerateCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\EventMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ExceptionMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\Factories\\FactoryMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\InterfaceMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\JobMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\JobMiddlewareMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\LangPublishCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ListenerMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\MailMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Routing\\Console\\MiddlewareMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ModelMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\NotificationMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Notifications\\Console\\NotificationTableCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ObserverMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\PolicyMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ProviderMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\FailedTableCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\TableCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Queue\\Console\\BatchesTableCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\RequestMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ResourceMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\RuleMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ScopeMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\Seeds\\SeederMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Session\\Console\\SessionTableCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ServeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\StubPublishCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\TestMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\TraitMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\VendorPublishCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Foundation\\Console\\ViewMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'migrator' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'migration.repository' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'migration.creator' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\Migrations\\MigrateCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\Migrations\\FreshCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\Migrations\\InstallCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\Migrations\\RefreshCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\Migrations\\ResetCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\Migrations\\RollbackCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\Migrations\\StatusCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'Illuminate\\Database\\Console\\Migrations\\MigrateMakeCommand' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'composer' => 'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider',
    'hash' => 'Illuminate\\Hashing\\HashServiceProvider',
    'hash.driver' => 'Illuminate\\Hashing\\HashServiceProvider',
    'mail.manager' => 'Illuminate\\Mail\\MailServiceProvider',
    'mailer' => 'Illuminate\\Mail\\MailServiceProvider',
    'Illuminate\\Mail\\Markdown' => 'Illuminate\\Mail\\MailServiceProvider',
    'Illuminate\\Contracts\\Pipeline\\Hub' => 'Illuminate\\Pipeline\\PipelineServiceProvider',
    'pipeline' => 'Illuminate\\Pipeline\\PipelineServiceProvider',
    'queue' => 'Illuminate\\Queue\\QueueServiceProvider',
    'queue.connection' => 'Illuminate\\Queue\\QueueServiceProvider',
    'queue.failer' => 'Illuminate\\Queue\\QueueServiceProvider',
    'queue.listener' => 'Illuminate\\Queue\\QueueServiceProvider',
    'queue.worker' => 'Illuminate\\Queue\\QueueServiceProvider',
    'redis' => 'Illuminate\\Redis\\RedisServiceProvider',
    'redis.connection' => 'Illuminate\\Redis\\RedisServiceProvider',
    'auth.password' => 'Illuminate\\Auth\\Passwords\\PasswordResetServiceProvider',
    'auth.password.broker' => 'Illuminate\\Auth\\Passwords\\PasswordResetServiceProvider',
    'translator' => 'Illuminate\\Translation\\TranslationServiceProvider',
    'translation.loader' => 'Illuminate\\Translation\\TranslationServiceProvider',
    'validator' => 'Illuminate\\Validation\\ValidationServiceProvider',
    'validation.presence' => 'Illuminate\\Validation\\ValidationServiceProvider',
    'Illuminate\\Contracts\\Validation\\UncompromisedVerifier' => 'Illuminate\\Validation\\ValidationServiceProvider',
  ),
  'when' => 
  array (
    'Illuminate\\Broadcasting\\BroadcastServiceProvider' => 
    array (
    ),
    'Illuminate\\Bus\\BusServiceProvider' => 
    array (
    ),
    'Illuminate\\Cache\\CacheServiceProvider' => 
    array (
    ),
    'Illuminate\\Foundation\\Providers\\ConsoleSupportServiceProvider' => 
    array (
    ),
    'Illuminate\\Hashing\\HashServiceProvider' => 
    array (
    ),
    'Illuminate\\Mail\\MailServiceProvider' => 
    array (
    ),
    'Illuminate\\Pipeline\\PipelineServiceProvider' => 
    array (
    ),
    'Illuminate\\Queue\\QueueServiceProvider' => 
    array (
    ),
    'Illuminate\\Redis\\RedisServiceProvider' => 
    array (
    ),
    'Illuminate\\Auth\\Passwords\\PasswordResetServiceProvider' => 
    array (
    ),
    'Illuminate\\Translation\\TranslationServiceProvider' => 
    array (
    ),
    'Illuminate\\Validation\\ValidationServiceProvider' => 
    array (
    ),
  ),
);
```

---

## config/app.php

```php
<?php

return [
    'name' => env('APP_NAME', 'PostHog Laravel Example'),
    'env' => env('APP_ENV', 'production'),
    'debug' => (bool) env('APP_DEBUG', false),
    'url' => env('APP_URL', 'http://localhost'),
    'timezone' => 'UTC',
    'locale' => 'en',
    'fallback_locale' => 'en',
    'key' => env('APP_KEY'),
    'cipher' => 'AES-256-CBC',

    'providers' => [
        // Laravel Framework Service Providers
        Illuminate\Auth\AuthServiceProvider::class,
        Illuminate\Broadcasting\BroadcastServiceProvider::class,
        Illuminate\Bus\BusServiceProvider::class,
        Illuminate\Cache\CacheServiceProvider::class,
        Illuminate\Foundation\Providers\ConsoleSupportServiceProvider::class,
        Illuminate\Cookie\CookieServiceProvider::class,
        Illuminate\Database\DatabaseServiceProvider::class,
        Illuminate\Encryption\EncryptionServiceProvider::class,
        Illuminate\Filesystem\FilesystemServiceProvider::class,
        Illuminate\Foundation\Providers\FoundationServiceProvider::class,
        Illuminate\Hashing\HashServiceProvider::class,
        Illuminate\Mail\MailServiceProvider::class,
        Illuminate\Notifications\NotificationServiceProvider::class,
        Illuminate\Pagination\PaginationServiceProvider::class,
        Illuminate\Pipeline\PipelineServiceProvider::class,
        Illuminate\Queue\QueueServiceProvider::class,
        Illuminate\Redis\RedisServiceProvider::class,
        Illuminate\Auth\Passwords\PasswordResetServiceProvider::class,
        Illuminate\Session\SessionServiceProvider::class,
        Illuminate\Translation\TranslationServiceProvider::class,
        Illuminate\Validation\ValidationServiceProvider::class,
        Illuminate\View\ViewServiceProvider::class,
    ],

    'aliases' => [
        'App' => Illuminate\Support\Facades\App::class,
        'Auth' => Illuminate\Support\Facades\Auth::class,
        'Blade' => Illuminate\Support\Facades\Blade::class,
        'Cache' => Illuminate\Support\Facades\Cache::class,
        'Config' => Illuminate\Support\Facades\Config::class,
        'DB' => Illuminate\Support\Facades\DB::class,
        'Hash' => Illuminate\Support\Facades\Hash::class,
        'Request' => Illuminate\Support\Facades\Request::class,
        'Route' => Illuminate\Support\Facades\Route::class,
        'Schema' => Illuminate\Support\Facades\Schema::class,
        'Session' => Illuminate\Support\Facades\Session::class,
        'View' => Illuminate\Support\Facades\View::class,
    ],
];

```

---

## config/auth.php

```php
<?php

return [
    'defaults' => [
        'guard' => 'web',
        'passwords' => 'users',
    ],

    'guards' => [
        'web' => [
            'driver' => 'session',
            'provider' => 'users',
        ],
    ],

    'providers' => [
        'users' => [
            'driver' => 'eloquent',
            'model' => App\Models\User::class,
        ],
    ],

    'passwords' => [
        'users' => [
            'provider' => 'users',
            'table' => 'password_reset_tokens',
            'expire' => 60,
            'throttle' => 60,
        ],
    ],

    'password_timeout' => 10800,
];

```

---

## config/database.php

```php
<?php

return [
    'default' => env('DB_CONNECTION', 'sqlite'),

    'connections' => [
        'sqlite' => [
            'driver' => 'sqlite',
            'url' => env('DATABASE_URL'),
            'database' => env('DB_DATABASE', database_path('database.sqlite')),
            'prefix' => '',
            'foreign_key_constraints' => env('DB_FOREIGN_KEYS', true),
        ],

        'mysql' => [
            'driver' => 'mysql',
            'url' => env('DATABASE_URL'),
            'host' => env('DB_HOST', '127.0.0.1'),
            'port' => env('DB_PORT', '3306'),
            'database' => env('DB_DATABASE', 'forge'),
            'username' => env('DB_USERNAME', 'forge'),
            'password' => env('DB_PASSWORD', ''),
            'charset' => 'utf8mb4',
            'collation' => 'utf8mb4_unicode_ci',
            'prefix' => '',
            'strict' => true,
            'engine' => null,
        ],
    ],

    'migrations' => 'migrations',
];

```

---

## config/posthog.php

```php
<?php

return [
    'api_key' => env('POSTHOG_API_KEY', ''),
    'host' => env('POSTHOG_HOST', 'https://us.i.posthog.com'),
    'disabled' => env('POSTHOG_DISABLED', false),
    'debug' => env('APP_DEBUG', false),
];

```

---

## config/session.php

```php
<?php

return [
    'driver' => env('SESSION_DRIVER', 'file'),
    'lifetime' => env('SESSION_LIFETIME', 120),
    'expire_on_close' => false,
    'encrypt' => false,
    'files' => storage_path('framework/sessions'),
    'connection' => null,
    'table' => 'sessions',
    'store' => null,
    'lottery' => [2, 100],
    'cookie' => env('SESSION_COOKIE', 'laravel_session'),
    'path' => '/',
    'domain' => env('SESSION_DOMAIN'),
    'secure' => env('SESSION_SECURE_COOKIE'),
    'http_only' => true,
    'same_site' => 'lax',
];

```

---

## database/migrations/2024_01_01_000000_create_users_table.php

```php
<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('users', function (Blueprint $table) {
            $table->id();
            $table->string('email')->unique();
            $table->string('password');
            $table->boolean('is_staff')->default(false);
            $table->rememberToken();
            $table->timestamps();
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('users');
    }
};

```

---

## database/seeders/DatabaseSeeder.php

```php
<?php

namespace Database\Seeders;

use App\Models\User;
use Illuminate\Database\Seeder;

class DatabaseSeeder extends Seeder
{
    public function run(): void
    {
        User::firstOrCreate(
            ['email' => 'admin@example.com'],
            [
                'password' => bcrypt('admin'),
                'is_staff' => true,
            ]
        );
    }
}

```

---

## IMPLEMENTATION.md

# Laravel PostHog Example - Implementation Summary

This document summarizes the implementation of the Laravel PostHog example application, ported from the Flask version.

## ✅ Completed Implementation

### Core Application Structure

**Models & Database**
- ✅ User model with PostHog properties helper method
- ✅ User migration with `is_staff` field
- ✅ Database seeder for default admin user
- ✅ SQLite database configuration

**PostHog Integration**
- ✅ PostHog configuration file (`config/posthog.php`)
- ✅ PostHogService class with all core methods:
  - `identify()` - User identification
  - `capture()` - Event tracking
  - `captureException()` - Error tracking
  - `isFeatureEnabled()` - Feature flag checking
  - `getFeatureFlagPayload()` - Feature flag payload retrieval

**Authentication (Livewire Components)**
- ✅ Login component with PostHog tracking
- ✅ Register component with PostHog tracking
- ✅ Logout route with PostHog tracking

**Core Features (Livewire Components)**
- ✅ Dashboard - Feature flag demonstration
- ✅ Burrito Tracker - Custom event tracking
- ✅ Profile - Error tracking demonstration

**API Controllers**
- ✅ BurritoController - API endpoint for burrito tracking
- ✅ ErrorTestController - Manual error capture demonstration

**Views & Layouts**
- ✅ App layout (authenticated users)
- ✅ Guest layout (unauthenticated users)
- ✅ All Livewire view files with inline styling
- ✅ Error pages (404, 500)

**Routes**
- ✅ Web routes (authentication, dashboard, burrito, profile, logout)
- ✅ API routes (burrito tracking, error testing)

**Configuration**
- ✅ Environment example file
- ✅ Composer.json with dependencies
- ✅ Laravel config files (app, auth, database, session)
- ✅ .gitignore

**Documentation**
- ✅ Comprehensive README
- ✅ Implementation plan (php-plan.md)

## 📋 Features Implemented

### 1. User Authentication
- Login with PostHog identification
- Registration with PostHog tracking
- Logout with event capture
- Session management

### 2. PostHog Analytics
- User identification on login/signup
- Person properties (email, is_staff, date_joined)
- Custom event tracking (burrito considerations)
- Dashboard views tracking

### 3. Feature Flags
- Feature flag checking (`new-dashboard-feature`)
- Feature flag payload retrieval
- Conditional UI rendering based on flags

### 4. Error Tracking
- Manual exception capture
- Error ID generation
- Test endpoint with optional capture (`?capture=true/false`)

### 5. UI/UX
- Responsive layouts
- Flash messages for user feedback
- Livewire reactivity for burrito counter
- Loading states on buttons

## 🎯 PostHog Integration Points

| Feature | Location | PostHog Method |
|---------|----------|----------------|
| User Login | `Login.php:23-27` | `identify()` + `capture()` |
| User Signup | `Register.php:29-32` | `identify()` + `capture()` |
| User Logout | `web.php:25` | `capture()` |
| Dashboard View | `Dashboard.php:18` | `capture()` |
| Feature Flag Check | `Dashboard.php:21-25` | `isFeatureEnabled()` |
| Feature Flag Payload | `Dashboard.php:28-31` | `getFeatureFlagPayload()` |
| Burrito Tracking | `BurritoTracker.php:22-24` | `identify()` + `capture()` |
| Profile View | `Profile.php:14` | `capture()` |
| Error Capture | `ErrorTestController.php:22-24` | `identify()` + `captureException()` |

## 📁 File Structure

```
basics/laravel/
├── app/
│   ├── Http/
│   │   ├── Controllers/
│   │   │   ├── Controller.php
│   │   │   └── Api/
│   │   │       ├── BurritoController.php
│   │   │       └── ErrorTestController.php
│   │   └── Livewire/
│   │       ├── Auth/
│   │       │   ├── Login.php
│   │       │   └── Register.php
│   │       ├── Dashboard.php
│   │       ├── BurritoTracker.php
│   │       └── Profile.php
│   ├── Models/
│   │   └── User.php
│   └── Services/
│       └── PostHogService.php
├── config/
│   ├── app.php
│   ├── auth.php
│   ├── database.php
│   ├── posthog.php
│   └── session.php
├── database/
│   ├── migrations/
│   │   └── 2024_01_01_000000_create_users_table.php
│   └── seeders/
│       └── DatabaseSeeder.php
├── resources/
│   └── views/
│       ├── components/
│       │   └── layouts/
│       │       ├── app.blade.php
│       │       └── guest.blade.php
│       ├── livewire/
│       │   ├── auth/
│       │   │   ├── login.blade.php
│       │   │   └── register.blade.php
│       │   ├── dashboard.blade.php
│       │   ├── burrito-tracker.blade.php
│       │   └── profile.blade.php
│       └── errors/
│           ├── 404.blade.php
│           └── 500.blade.php
├── routes/
│   ├── api.php
│   └── web.php
├── .env.example
├── .gitignore
├── composer.json
├── IMPLEMENTATION.md
└── README.md
```

## 🔄 Flask to Laravel Mapping

| Flask Component | Laravel Equivalent |
|----------------|-------------------|
| Flask-Login | Laravel Auth + Livewire |
| Flask-SQLAlchemy | Eloquent ORM |
| Jinja2 Templates | Blade Templates + Livewire |
| Blueprint routes | Route definitions |
| @app.route decorators | Route::get/post |
| session | session() helper |
| flash() | session()->flash() |
| @login_required | Route::middleware('auth') |
| request.form | Livewire properties |
| render_template() | view() or Livewire render() |
| jsonify() | response()->json() |
| SQLAlchemy models | Eloquent models |

## 🚀 Next Steps for Production

To make this a production-ready application:

1. **Install via Composer**: Run full Laravel installation
2. **Environment**: Generate APP_KEY with `php artisan key:generate`
3. **Database**: Run migrations with `php artisan migrate --seed`
4. **Assets**: Set up Vite for asset compilation
5. **Middleware**: Add CSRF protection middleware
6. **Validation**: Add form request classes
7. **Testing**: Implement PHPUnit tests
8. **Caching**: Configure Redis/Memcached
9. **Queue**: Set up queue workers for PostHog events
10. **Deployment**: Configure for production server

## 📝 Notes

- This implementation uses inline CSS (matching Flask example) instead of Tailwind compilation
- Livewire provides reactivity without separate JavaScript files
- PostHog service is dependency-injected into components/controllers
- Manual error capture pattern matches Flask implementation
- Session-based burrito counter (same as Flask)
- Default admin account: admin@example.com / admin

## 🎓 Learning Resources

- [Laravel Documentation](https://laravel.com/docs)
- [Livewire Documentation](https://livewire.laravel.com)
- [PostHog PHP SDK](https://github.com/PostHog/posthog-php)
- [Eloquent ORM](https://laravel.com/docs/eloquent)

---

**Implementation Date**: January 2026
**Laravel Version**: 11.x
**Livewire Version**: 3.x
**PostHog PHP SDK**: 3.x

---

## public/index.php

```php
<?php

use Illuminate\Http\Request;

define('LARAVEL_START', microtime(true));

// Suppress PHP 8.5 deprecation warnings for development
error_reporting(E_ALL & ~E_DEPRECATED);

// Determine if the application is in maintenance mode...
if (file_exists($maintenance = __DIR__.'/../storage/framework/maintenance.php')) {
    require $maintenance;
}

// Register the Composer autoloader...
require __DIR__.'/../vendor/autoload.php';

// Bootstrap Laravel and handle the request...
(require_once __DIR__.'/../bootstrap/app.php')
    ->handleRequest(Request::capture());

```

---

## resources/views/components/layouts/app.blade.php

```php
<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{{ csrf_token() }}">

    <title>{{ $title ?? 'PostHog Laravel Example' }}</title>

    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            line-height: 1.6;
            background-color: #f5f5f5;
            color: #333;
        }
        .container {
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
        }
        nav {
            background: #1d4ed8;
            padding: 15px 20px;
            margin-bottom: 30px;
        }
        nav a {
            color: white;
            text-decoration: none;
            margin-right: 20px;
        }
        nav a:hover {
            text-decoration: underline;
        }
        .nav-right {
            float: right;
        }
        .card {
            background: white;
            border-radius: 8px;
            padding: 20px;
            margin-bottom: 20px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        h1, h2, h3 {
            margin-bottom: 15px;
            color: #1d4ed8;
        }
        button, .btn {
            background: #1d4ed8;
            color: white;
            border: none;
            padding: 10px 20px;
            border-radius: 5px;
            cursor: pointer;
            font-size: 14px;
            display: inline-block;
            text-decoration: none;
        }
        button:hover, .btn:hover {
            background: #1e40af;
        }
        button:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        button.danger, .btn-danger {
            background: #dc2626;
        }
        button.danger:hover, .btn-danger:hover {
            background: #b91c1c;
        }
        input {
            width: 100%;
            padding: 10px;
            margin-bottom: 15px;
            border: 1px solid #ddd;
            border-radius: 5px;
            font-size: 14px;
        }
        .message-error {
            background: #fee2e2;
            color: #dc2626;
            padding: 10px 15px;
            border-radius: 5px;
            margin-bottom: 15px;
        }
        .message-success {
            background: #d1fae5;
            color: #059669;
            padding: 10px 15px;
            border-radius: 5px;
            margin-bottom: 15px;
        }
        .feature-flag {
            background: #fef3c7;
            border: 2px dashed #f59e0b;
            padding: 15px;
            border-radius: 8px;
            margin: 20px 0;
        }
        code {
            background: #f3f4f6;
            padding: 2px 6px;
            border-radius: 3px;
            font-family: monospace;
        }
        pre {
            background: #1e293b;
            color: #e2e8f0;
            padding: 16px;
            border-radius: 8px;
            overflow-x: auto;
            font-size: 13px;
        }
        .count {
            font-size: 48px;
            font-weight: bold;
            color: #1d4ed8;
            text-align: center;
            padding: 20px;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            margin: 16px 0;
        }
        th, td {
            padding: 12px;
            text-align: left;
            border-bottom: 1px solid #eee;
        }
        th {
            background: #f8fafc;
            font-weight: 600;
        }
        label {
            display: block;
            margin-bottom: 5px;
            font-weight: 500;
        }
        .text-sm {
            font-size: 14px;
        }
        .text-gray {
            color: #666;
        }
        .mb-4 {
            margin-bottom: 16px;
        }
    </style>
    @livewireStyles
</head>
<body>
    @auth
    <nav>
        <a href="{{ route('dashboard') }}">Dashboard</a>
        <a href="{{ route('burrito') }}">Burrito</a>
        <a href="{{ route('profile') }}">Profile</a>
        <span class="nav-right">
            <span style="margin-right: 15px;">{{ auth()->user()->email }}</span>
            <form method="POST" action="{{ route('logout') }}" style="display: inline;">
                @csrf
                <button type="submit" style="background: none; padding: 0; color: white; text-decoration: underline;">Logout</button>
            </form>
        </span>
    </nav>
    @endauth

    <div class="container">
        @if (session('success'))
            <div class="message-success">
                {{ session('success') }}
            </div>
        @endif

        @if (session('error'))
            <div class="message-error">
                {{ session('error') }}
            </div>
        @endif

        {{ $slot }}
    </div>

    @livewireScripts
</body>
</html>

```

---

## resources/views/components/layouts/guest.blade.php

```php
<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{{ csrf_token() }}">

    <title>{{ $title ?? 'PostHog Laravel Example' }}</title>

    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            line-height: 1.6;
            background-color: #f5f5f5;
            color: #333;
        }
        .container {
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
        }
        .card {
            background: white;
            border-radius: 8px;
            padding: 20px;
            margin-bottom: 20px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        h1, h2, h3 {
            margin-bottom: 15px;
            color: #1d4ed8;
        }
        button, .btn {
            background: #1d4ed8;
            color: white;
            border: none;
            padding: 10px 20px;
            border-radius: 5px;
            cursor: pointer;
            font-size: 14px;
            display: inline-block;
            text-decoration: none;
            width: 100%;
        }
        button:hover, .btn:hover {
            background: #1e40af;
        }
        input {
            width: 100%;
            padding: 10px;
            margin-bottom: 15px;
            border: 1px solid #ddd;
            border-radius: 5px;
            font-size: 14px;
        }
        label {
            display: block;
            margin-bottom: 5px;
            font-weight: 500;
        }
        .error {
            color: #dc2626;
            font-size: 13px;
            margin-top: -10px;
            margin-bottom: 10px;
        }
        .text-sm {
            font-size: 14px;
        }
        .text-gray {
            color: #666;
        }
        a {
            color: #1d4ed8;
        }
        code {
            background: #f3f4f6;
            padding: 2px 6px;
            border-radius: 3px;
            font-family: monospace;
            font-size: 13px;
        }
        pre {
            background: #1e293b;
            color: #e2e8f0;
            padding: 16px;
            border-radius: 8px;
            overflow-x: auto;
            font-size: 13px;
            margin-top: 10px;
        }
        ul {
            margin-left: 20px;
        }
    </style>
    @livewireStyles
</head>
<body>
    <div class="container">
        {{ $slot }}
    </div>

    @livewireScripts
</body>
</html>

```

---

## resources/views/errors/404.blade.php

```php
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Page Not Found - PostHog Laravel Example</title>
    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            line-height: 1.6;
            background-color: #f5f5f5;
            color: #333;
        }
        .container {
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
        }
        .card {
            background: white;
            border-radius: 8px;
            padding: 40px;
            margin-top: 50px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            text-align: center;
        }
        h1 {
            font-size: 72px;
            color: #1d4ed8;
            margin-bottom: 15px;
        }
        h2 {
            font-size: 24px;
            color: #333;
            margin-bottom: 15px;
        }
        p {
            color: #666;
            margin-bottom: 25px;
        }
        .btn {
            background: #1d4ed8;
            color: white;
            border: none;
            padding: 12px 24px;
            border-radius: 5px;
            cursor: pointer;
            font-size: 14px;
            text-decoration: none;
            display: inline-block;
        }
        .btn:hover {
            background: #1e40af;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="card">
            <h1>404</h1>
            <h2>Page Not Found</h2>
            <p>The page you're looking for doesn't exist.</p>
            <a href="/" class="btn">Go Home</a>
        </div>
    </div>
</body>
</html>

```

---

## resources/views/errors/500.blade.php

```php
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Server Error - PostHog Laravel Example</title>
    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            line-height: 1.6;
            background-color: #f5f5f5;
            color: #333;
        }
        .container {
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
        }
        .card {
            background: white;
            border-radius: 8px;
            padding: 40px;
            margin-top: 50px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            text-align: center;
        }
        h1 {
            font-size: 72px;
            color: #dc2626;
            margin-bottom: 15px;
        }
        h2 {
            font-size: 24px;
            color: #333;
            margin-bottom: 15px;
        }
        p {
            color: #666;
            margin-bottom: 25px;
        }
        .btn {
            background: #1d4ed8;
            color: white;
            border: none;
            padding: 12px 24px;
            border-radius: 5px;
            cursor: pointer;
            font-size: 14px;
            text-decoration: none;
            display: inline-block;
        }
        .btn:hover {
            background: #1e40af;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="card">
            <h1>500</h1>
            <h2>Internal Server Error</h2>
            <p>Something went wrong on our end.</p>
            <a href="/" class="btn">Go Home</a>
        </div>
    </div>
</body>
</html>

```

---

## resources/views/livewire/auth/login.blade.php

```php
<div>
    <div class="card">
        <h1>Welcome to PostHog Laravel Example</h1>
        <p class="text-gray mb-4">This example demonstrates how to integrate PostHog with a Laravel application.</p>

        <form wire:submit="login">
            <label for="email">Email</label>
            <input
                type="email"
                id="email"
                wire:model="email"
                required
            >
            @error('email') <div class="error">{{ $message }}</div> @enderror

            <label for="password">Password</label>
            <input
                type="password"
                id="password"
                wire:model="password"
                required
            >
            @error('password') <div class="error">{{ $message }}</div> @enderror

            <div style="margin-bottom: 15px;">
                <label style="display: inline; font-weight: normal;">
                    <input type="checkbox" wire:model="remember" style="width: auto; margin-right: 5px;">
                    Remember me
                </label>
            </div>

            <button type="submit">Login</button>
        </form>

        <p style="margin-top: 16px;" class="text-sm text-gray">
            Don't have an account? <a href="{{ route('register') }}">Sign up here</a>
        </p>
        <p class="text-sm text-gray">
            <strong>Tip:</strong> Default credentials are admin@example.com/admin
        </p>
    </div>

    <div class="card">
        <h2>Features Demonstrated</h2>
        <ul class="text-gray">
            <li>User registration and identification</li>
            <li>Event tracking</li>
            <li>Feature flags</li>
            <li>Error tracking</li>
        </ul>
    </div>
</div>

```

---

## resources/views/livewire/auth/register.blade.php

```php
<div>
    <div class="card">
        <h1>Create an Account</h1>
        <p class="text-gray mb-4">Sign up to explore the PostHog Laravel integration example.</p>

        <form wire:submit="register">
            <label for="email">Email *</label>
            <input
                type="email"
                id="email"
                wire:model="email"
                required
            >
            @error('email') <div class="error">{{ $message }}</div> @enderror

            <label for="password">Password *</label>
            <input
                type="password"
                id="password"
                wire:model="password"
                required
            >
            @error('password') <div class="error">{{ $message }}</div> @enderror

            <label for="password_confirmation">Confirm Password *</label>
            <input
                type="password"
                id="password_confirmation"
                wire:model="password_confirmation"
                required
            >

            <button type="submit">Sign Up</button>
        </form>

        <p style="margin-top: 16px;" class="text-sm text-gray">
            Already have an account? <a href="{{ route('login') }}">Login here</a>
        </p>
    </div>

    <div class="card">
        <h2>PostHog Integration</h2>
        <p class="text-gray">When you sign up, the following PostHog events are captured:</p>
        <ul class="text-gray" style="margin-top: 10px;">
            <li><code>identify()</code> - Associates your email with the user</li>
            <li><code>capture()</code> - Sets person properties (email, etc.)</li>
            <li><code>user_signed_up</code> event - Tracks the signup action</li>
        </ul>

        <h3 style="margin-top: 20px;">Code Example</h3>
        <pre>// After creating the user
$posthog->identify($user->email, $user->getPostHogProperties());
$posthog->capture($user->email, 'user_signed_up', [
    'signup_method' => 'form'
]);</pre>
    </div>
</div>

```

---

## resources/views/livewire/burrito-tracker.blade.php

```php
<div>
    <div class="card">
        <h1>Burrito Consideration Tracker</h1>
        <p class="text-gray mb-4">This page demonstrates custom event tracking with PostHog.</p>

        <div class="count">{{ $burritoCount }}</div>
        <p style="text-align: center; color: #666; margin-bottom: 20px;">Times you've considered a burrito</p>

        <div style="text-align: center;">
            <button
                wire:click="considerBurrito"
                wire:loading.attr="disabled"
            >
                <span wire:loading.remove>Consider a Burrito</span>
                <span wire:loading>Considering...</span>
            </button>
        </div>
    </div>

    <div class="card">
        <h3>Code Example</h3>
        <pre>// Livewire component method
public function considerBurrito(PostHogService $posthog)
{
    $this->burritoCount++;
    session(['burrito_count' => $this->burritoCount]);

    $user = Auth::user();
    $posthog->identify($user->email, $user->getPostHogProperties());
    $posthog->capture($user->email, 'burrito_considered', [
        'total_considerations' => $this->burritoCount,
    ]);
}</pre>
    </div>
</div>

```

---

## resources/views/livewire/dashboard.blade.php

```php
<div>
    <div class="card">
        <h1>Dashboard</h1>
        <p class="text-gray">Welcome back, {{ auth()->user()->email }}!</p>
    </div>

    <div class="card">
        <h2>Error Tracking Demo</h2>
        <p class="text-gray">Test manual exception capture in PostHog. These buttons trigger errors in the context of your logged-in user.</p>

        @if($successMessage)
            <div style="background: #d4edda; border: 1px solid #c3e6cb; color: #155724; padding: 12px; border-radius: 4px; margin: 15px 0;">
                {{ $successMessage }}
            </div>
        @endif

        @if($errorMessage)
            <div style="background: #f8d7da; border: 1px solid #f5c6cb; color: #721c24; padding: 12px; border-radius: 4px; margin: 15px 0;">
                {{ $errorMessage }}
            </div>
        @endif

        <div style="display: flex; gap: 10px; margin-top: 15px;">
            <button wire:click="testErrorWithCapture" class="btn" style="background: #dc3545; color: white;">
                Capture Error in PostHog
            </button>
            <button wire:click="testErrorWithoutCapture" class="btn" style="background: #c82333; color: white;">
                Skip PostHog Capture
            </button>
        </div>

        <h3 style="margin-top: 20px;">Code Example</h3>
        <pre>try {
    // Critical operation that might fail
    processPayment();
} catch (\Throwable $e) {
    // Manually capture this specific exception
    $errorId = $posthog->captureException($e, $user->email);

    return response()->json([
        'error' => 'Operation failed',
        'error_id' => $errorId
    ], 500);
}</pre>
        <p class="text-gray" style="margin-top: 10px;">This demonstrates manual exception capture where you have control over whether errors are sent to PostHog.</p>
    </div>

    <div class="card">
        <h2>Feature Flags</h2>

        @if($showNewFeature)
            <div class="feature-flag">
                <strong>New Feature Enabled!</strong>
                <p style="margin-top: 10px;">You're seeing this because the <code>new-dashboard-feature</code> flag is enabled for you.</p>

                @if($featureConfig)
                    <p style="margin-top: 15px;"><strong>Feature Configuration:</strong></p>
                    <pre>{{ json_encode($featureConfig, JSON_PRETTY_PRINT) }}</pre>
                @endif
            </div>
        @else
            <p class="text-gray">The <code>new-dashboard-feature</code> flag is not enabled for your account.</p>
        @endif

        <h3 style="margin-top: 20px;">Code Example</h3>
        <pre>// Check if feature flag is enabled
$showNewFeature = $posthog->isFeatureEnabled(
    'new-dashboard-feature',
    $user->email,
    $user->getPostHogProperties()
);

// Get feature flag payload
$featureConfig = $posthog->getFeatureFlagPayload(
    'new-dashboard-feature',
    $user->email
);</pre>
    </div>

</div>

```

---

## resources/views/livewire/profile.blade.php

```php
<div>
    <div class="card">
        <h1>Your Profile</h1>
        <p class="text-gray mb-4">This page demonstrates error tracking with PostHog.</p>

        <table>
            <tr>
                <th>Email</th>
                <td>{{ auth()->user()->email }}</td>
            </tr>
            <tr>
                <th>Date Joined</th>
                <td>{{ auth()->user()->created_at->format('Y-m-d H:i') }}</td>
            </tr>
            <tr>
                <th>Staff Status</th>
                <td>{{ auth()->user()->is_staff ? 'Yes' : 'No' }}</td>
            </tr>
        </table>
    </div>

    <div class="card">
        <h2>Error Tracking Demo</h2>
        <p class="text-gray">Test manual exception capture in PostHog. These buttons trigger errors in the context of your logged-in user.</p>

        @if($successMessage)
            <div style="background: #d4edda; border: 1px solid #c3e6cb; color: #155724; padding: 12px; border-radius: 4px; margin: 15px 0;">
                {{ $successMessage }}
            </div>
        @endif

        @if($errorMessage)
            <div style="background: #f8d7da; border: 1px solid #f5c6cb; color: #721c24; padding: 12px; border-radius: 4px; margin: 15px 0;">
                {{ $errorMessage }}
            </div>
        @endif

        <div style="display: flex; gap: 10px; margin-top: 15px;">
            <button wire:click="testErrorWithCapture" class="btn" style="background: #dc3545; color: white;">
                Capture Error in PostHog
            </button>
            <button wire:click="testErrorWithoutCapture" class="btn" style="background: #c82333; color: white;">
                Skip PostHog Capture
            </button>
        </div>

        <p class="text-gray" style="margin-top: 15px;">
            This demonstrates manual exception capture where you have control over whether errors are sent to PostHog.
        </p>
    </div>

    <div class="card">
        <h3>Code Example</h3>
        <pre>try {
    throw new \Exception('Test exception from critical operation');
} catch (\Throwable $e) {
    // Capture exception with user context
    $posthog->identify($user->email, $user->getPostHogProperties());
    $eventId = $posthog->captureException($e, $user->email);

    return response()->json([
        'error' => 'Operation failed',
        'error_id' => $eventId,
        'message' => "Error captured in PostHog. Reference ID: {$eventId}"
    ], 500);
}</pre>
    </div>
</div>

```

---

## routes/api.php

```php
<?php

use App\Http\Controllers\Api\BurritoController;
use App\Http\Controllers\Api\ErrorTestController;
use Illuminate\Support\Facades\Route;

Route::middleware('auth:sanctum')->group(function () {
    Route::post('/burrito/consider', [BurritoController::class, 'consider']);
    Route::post('/test-error', [ErrorTestController::class, 'test']);
});

```

---

## routes/web.php

```php
<?php

use App\Http\Livewire\Auth\Login;
use App\Http\Livewire\Auth\Register;
use App\Http\Livewire\BurritoTracker;
use App\Http\Livewire\Dashboard;
use App\Http\Livewire\Profile;
use App\Services\PostHogService;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Route;

// Guest routes
Route::middleware('guest')->group(function () {
    Route::get('/', Login::class)->name('login');
    Route::get('/register', Register::class)->name('register');
});

// Authenticated routes
Route::middleware('auth')->group(function () {
    Route::get('/dashboard', Dashboard::class)->name('dashboard');
    Route::get('/burrito', BurritoTracker::class)->name('burrito');
    Route::get('/profile', Profile::class)->name('profile');

    Route::post('/logout', function (PostHogService $posthog) {
        $user = Auth::user();

        // PostHog: Track logout
        $posthog->capture($user->email, 'user_logged_out');

        Auth::logout();
        request()->session()->invalidate();
        request()->session()->regenerateToken();

        return redirect('/');
    })->name('logout');
});

```

---

