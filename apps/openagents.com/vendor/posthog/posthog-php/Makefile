dependencies: vendor

vendor: composer.phar
	@php ./composer.phar install

composer.phar:
	@curl -sS https://getcomposer.org/installer | php

docker:
	@docker run -it --rm -v ${PWD}:/app -w /app chialab/php:7.4 bash

test: lint
	@vendor/bin/phpunit --colors test/
	@php ./composer.phar validate

lint: dependencies
	@if php -r 'exit(version_compare(PHP_VERSION, "5.5", ">=") ? 0 : 1);'; \
	then \
		php ./composer.phar require overtrue/phplint --dev; \
		php ./composer.phar require squizlabs/php_codesniffer --dev; \
		./vendor/bin/phplint; \
		./vendor/bin/phpcs; \
	else \
		printf "Please update PHP version to 5.5 or above for code formatting."; \
	fi

release:
	@printf "releasing ${VERSION}..."
	@if [ "$(shell uname)" = "Darwin" ]; then \
		sed -i '' -E -e "s/public const VERSION = '[^']*'/public const VERSION = '${VERSION}'/" ./lib/PostHog.php; \
	else \
		sed -i -E -e "s/public const VERSION = '[^']*'/public const VERSION = '${VERSION}'/" ./lib/PostHog.php; \
	fi
	@node -e "var fs = require('fs'), pkg = require('./composer'); pkg.version = '${VERSION}'; fs.writeFileSync('./composer.json', JSON.stringify(pkg, null, '\t'));"
	@git changelog -t ${VERSION}
	@git release ${VERSION}
	@gh release create ${VERSION} --generate-notes 

clean:
	rm -rf \
		composer.phar \
		vendor \
		composer.lock
