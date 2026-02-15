# syntax=docker/dockerfile:1

FROM php:8.2-cli-bookworm AS php_deps

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
RUN composer install --no-dev --no-interaction --no-progress --prefer-dist --optimize-autoloader


FROM node:22-bookworm AS node_build

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . ./
RUN npm run build


FROM php:8.2-fpm-bookworm AS runtime

WORKDIR /var/www/app

RUN apt-get update \
  && apt-get install -y --no-install-recommends nginx ca-certificates libpq-dev libsqlite3-dev \
  && rm -rf /var/lib/apt/lists/*

# PHP extensions commonly needed by Laravel
RUN docker-php-ext-install pdo pdo_pgsql pdo_sqlite

COPY --chown=www-data:www-data . ./
COPY --from=php_deps /app/vendor ./vendor
COPY --from=node_build /app/public/build ./public/build

COPY deploy/nginx/nginx.conf /etc/nginx/nginx.conf
COPY deploy/start.sh /start.sh

RUN chmod +x /start.sh \
  && mkdir -p storage/framework/{cache,sessions,views} bootstrap/cache \
  && chown -R www-data:www-data storage bootstrap/cache

EXPOSE 8080

CMD ["/start.sh"]
