# Technology Stack

OpenAgents v3 uses a modern full-stack architecture. Here's a detailed breakdown of our technology choices:

## Backend Stack

### Core Framework
- **PHP 8.2+**
- **Laravel 11.x** - Modern PHP framework
- **Laravel Sanctum** - API authentication
- **Laravel Breeze** - Authentication scaffolding
- **Inertia.js** - Modern monolith architecture

### Development Tools
- **Laravel Pail** - Log viewer
- **Laravel Sail** - Docker development environment
- **Pest PHP** - Testing framework
- **Laravel Pint** - PHP code style fixer

### File Processing
- **spatie/pdf-to-text** - PDF text extraction

## Frontend Stack

### Core Framework
- **React 18** - UI framework
- **TypeScript** - Type-safe JavaScript
- **Vite** - Build tool and dev server

### UI Components
- **Radix UI** - Headless component primitives:
  - Avatar
  - Collapsible
  - Dialog
  - Dropdown Menu
  - Icons
  - Label
  - Separator
  - Tooltip
- **Tailwind CSS** - Utility-first CSS framework
- **HeadlessUI** - Additional React components

### Development Tools
- **TypeScript** - Static typing
- **class-variance-authority** - Component variants
- **clsx** - Conditional classes
- **tailwind-merge** - CSS class merging

### File Handling
- **react-dropzone** - File upload interface

## Development Environment

### Scripts
Available in package.json:
```bash
# Frontend
npm run dev     # Start Vite dev server
npm run build   # Build for production

# Backend (via composer.json)
composer dev    # Start all services (server, queue, logs, vite)
```

### Development Commands
```bash
# Start all services
composer dev

# Individual services
php artisan serve              # Laravel server
php artisan queue:listen       # Queue worker
php artisan pail              # Log viewer
npm run dev                   # Vite dev server
```