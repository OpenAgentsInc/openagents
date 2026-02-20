# syntax=docker/dockerfile:1

# Dependencies for PHP (Composer).
FROM php:8.4-cli-bookworm AS php_deps

WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends git unzip libzip-dev \
  && rm -rf /var/lib/apt/lists/*

RUN docker-php-ext-install zip

# Install Composer
RUN php -r "copy('https://getcomposer.org/installer', 'composer-setup.php');" \
  && php composer-setup.php --install-dir=/usr/local/bin --filename=composer \
  && rm composer-setup.php

COPY composer.json composer.lock ./
# Laravel's default Composer scripts expect `artisan` to exist.
# We skip scripts here for better layer caching and run discovery in the runtime stage.
RUN composer install --no-dev --no-interaction --no-progress --prefer-dist --optimize-autoloader --no-scripts


# Node runtime image (we copy only node/npm from here into the build stage).
FROM node:22-bookworm AS node_runtime


# Build JS assets (requires PHP to run `php artisan wayfinder:generate` during Vite build).
FROM php:8.4-cli-bookworm AS node_build

WORKDIR /app

# Bring in Node + npm without clobbering PHP's /usr/local.
COPY --from=node_runtime /usr/local /opt/node
ENV PATH="/opt/node/bin:$PATH"

COPY package.json package-lock.json ./
COPY packages/khala-sync /packages/khala-sync
RUN rm -rf /packages/khala-sync/node_modules
RUN npm ci || npm install

COPY . ./
COPY --from=php_deps /app/vendor ./vendor
RUN php scripts/patch-oooas.php

# The build context may include a stale discovery cache (e.g. generated with dev deps).
# Regenerate it for the production vendor set before any artisan commands run.
RUN rm -f bootstrap/cache/packages.php bootstrap/cache/services.php || true

RUN mkdir -p storage/framework/cache storage/framework/sessions storage/framework/views bootstrap/cache

RUN php -r 'file_put_contents(".env", "APP_KEY=base64:" . base64_encode(random_bytes(32)) . PHP_EOL . "APP_ENV=production" . PHP_EOL . "APP_DEBUG=0" . PHP_EOL);'

RUN php artisan package:discover --ansi
RUN APP_KEY="base64:$(php -r 'echo base64_encode(random_bytes(32));')" APP_ENV=production APP_DEBUG=0 APP_URL=https://openagents.com php artisan openapi:generate --output=public/openapi.json --ansi
RUN php scripts/normalize-openapi.php public/openapi.json
RUN test -s public/openapi.json

RUN npm run build


# Runtime image: Nginx + PHP-FPM
FROM php:8.4-fpm-bookworm AS runtime

WORKDIR /var/www/app

RUN apt-get update \
  && apt-get install -y --no-install-recommends nginx ca-certificates libpq-dev libsqlite3-dev $PHPIZE_DEPS \
  && docker-php-ext-install pdo pdo_pgsql pdo_sqlite \
  && pecl install redis \
  && docker-php-ext-enable redis \
  && apt-get purge -y --auto-remove $PHPIZE_DEPS \
  && rm -rf /var/lib/apt/lists/*

COPY --chown=www-data:www-data . ./
COPY --from=php_deps /app/vendor ./vendor
RUN php scripts/patch-oooas.php
COPY --from=node_build /app/public/build ./public/build
COPY --from=node_build /app/public/openapi.json ./public/openapi.json

# Generate the package discovery manifest in the image.
RUN rm -f bootstrap/cache/packages.php bootstrap/cache/services.php || true
RUN php artisan package:discover --ansi

COPY deploy/nginx/nginx.conf /etc/nginx/nginx.conf
COPY deploy/start.sh /start.sh

RUN chmod +x /start.sh \
  && mkdir -p storage/framework/cache storage/framework/sessions storage/framework/views bootstrap/cache \
  && chown -R www-data:www-data storage bootstrap/cache

EXPOSE 8080

CMD ["/start.sh"]
