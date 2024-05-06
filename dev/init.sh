#!/bin/bash
cd /openagents-pool
git fetch --all
git checkout $POOL_SNAPSHOT
npm i
npm run debug > /dev/stdout 2> /dev/stdout &


cd /app
npm i
composer install
npm run dev -- --host 0.0.0.0 > /dev/stdout 2> /dev/stdout &
service redis-server start
php artisan migrate -n
php artisan serve --host=0.0.0.0 > /dev/stdout 2> /dev/stdout &


tail -f /dev/stdout