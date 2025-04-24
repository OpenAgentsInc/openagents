# OpenAgents.com Development Guidelines

Note, you are in a monorepo. When making edits, stay within this apps/openagents.com/ folder unless directed otherwise.

## Build/Test Commands
- `composer install` - Install PHP dependencies
- `npm install` - Install JS dependencies
- `composer dev` - Run development server with concurrently (server, queue, logs, vite)
- `composer test` - Run all tests
- `php artisan test --filter=TestName` - Run a single test
- `npm run dev` - Start Vite development server
- `npm run build` - Build frontend assets
- `npm run lint` - Run ESLint and fix issues
- `npm run format` - Format code with Prettier
- `npm run types` - Run TypeScript type checking

## Code Style Guidelines
- PHP: PSR-12 standard, enforced with Laravel Pint
- TypeScript/React:
  - Use TypeScript for all components
  - Semi-colons required, single quotes preferred
  - Tab width: 4 spaces (2 for YAML files)
  - Max line length: 150 characters
  - Organize imports automatically with prettier-plugin-organize-imports
  - Follow React Hooks rules (dependencies array, etc.)
- Error Handling: Use Laravel's built-in exception handling
- CSS: Use Tailwind utilities with tailwindcss-plugin for automatic class sorting
- Testing: Pest PHP for backend, React Testing Library for frontend
