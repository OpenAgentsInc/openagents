# Project Structure

OpenAgents v3 is built with Laravel and React. This document outlines the key directories and their purposes.

## Backend (Laravel)

### `/app`
- `/Http/Controllers` - Request handlers and business logic
- `/Http/Middleware` - Request/response filters
- `/Http/Requests` - Form validation rules
- `/Models` - Eloquent models representing database tables
- `/Providers` - Service providers for Laravel's IoC container

### `/database`
- `/migrations` - Database structure definitions
- `/factories` - Test data generators
- `/seeders` - Initial data seeders

### `/routes`
- `web.php` - Web routes
- `api.php` - API routes

## Frontend (React + Inertia)

### `/resources/js`
- `/Components` - Reusable React components
- `/Layouts` - Page layout templates
- `/Pages` - Page components (mapped to routes)
- `/hooks` - Custom React hooks
- `/lib` - Utility functions and helpers
- `/types` - TypeScript type definitions

### `/resources/css`
- Tailwind CSS and custom styles

## Key Technologies

### Backend
- PHP 8.2+
- Laravel 11
- Inertia.js for frontend integration
- Sanctum for authentication
- Pest for testing

### Frontend
- React 18
- TypeScript
- Tailwind CSS
- Radix UI components
- Vite for building

### Development Tools
- Laravel Pail for log viewing
- Laravel Sail for Docker development
- Laravel Pint for code style
- Pest for PHP testing

## Development Setup

1. Clone the repository
2. Copy `.env.example` to `.env`
3. Install PHP dependencies:
   ```bash
   composer install
   ```
4. Install Node dependencies:
   ```bash
   npm install
   ```
5. Generate application key:
   ```bash
   php artisan key:generate
   ```
6. Run migrations:
   ```bash
   php artisan migrate
   ```
7. Start development servers:
   ```bash
   composer dev
   ```

This will start:
- Laravel development server
- Queue worker
- Log viewer
- Vite dev server