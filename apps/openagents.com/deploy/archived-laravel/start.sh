#!/usr/bin/env bash
set -euo pipefail

# Cloud Run sets PORT=8080. Keep the default Nginx config pinned to 8080.

mkdir -p storage/framework/{cache,sessions,views} bootstrap/cache
chown -R www-data:www-data storage bootstrap/cache || true

# Start PHP-FPM (background) then Nginx (foreground)
php-fpm -D
exec nginx -g 'daemon off;'
