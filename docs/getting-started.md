# Getting Started with OpenAgents

This guide will help you set up OpenAgents for local development.

## Prerequisites

- PHP 8.2 or higher
- Node.js 18 or higher
- Composer
- Git

## Initial Setup

1. **Clone the repository**
```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
```

2. **Install PHP dependencies**
```bash
composer install
```

3. **Install JavaScript dependencies**
```bash
npm install
```

4. **Environment Setup**
```bash
cp .env.example .env
php artisan key:generate
```

5. **Database Setup**
```bash
# For SQLite (default)
touch database/database.sqlite
php artisan migrate

# For other databases, update .env accordingly
DB_CONNECTION=mysql
DB_HOST=127.0.0.1
DB_PORT=3306
DB_DATABASE=openagents
DB_USERNAME=root
DB_PASSWORD=
```

## Development

1. **Start all services**
```bash
composer dev
```

This will start:
- Laravel development server
- Queue worker
- Log viewer
- Vite dev server

2. **Or start services individually**
```bash
# In separate terminals:
php artisan serve              # Laravel server
php artisan queue:listen       # Queue worker
php artisan pail              # Log viewer
npm run dev                   # Vite dev server
```

3. **Visit the application**
```
http://localhost:8000
```

## Testing

```bash
# Run all tests
php artisan test

# Run specific test file
php artisan test tests/Feature/ExampleTest.php
```

## Code Style

```bash
# Fix PHP code style
./vendor/bin/pint

# Check TypeScript types
npm run typecheck
```

## Common Issues

### Storage Permissions
If you encounter file permission issues:
```bash
php artisan storage:link
chmod -R 777 storage bootstrap/cache
```

### Database Issues
Reset the database if needed:
```bash
php artisan migrate:fresh --seed
```

### Cache Issues
Clear various caches:
```bash
php artisan optimize:clear
```