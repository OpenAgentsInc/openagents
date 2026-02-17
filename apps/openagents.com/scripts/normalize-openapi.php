<?php

declare(strict_types=1);

if ($argc < 2 || ! is_string($argv[1]) || trim($argv[1]) === '') {
    fwrite(STDERR, "usage: php scripts/normalize-openapi.php <path-to-openapi-json>\n");
    exit(1);
}

$path = $argv[1];
$raw = @file_get_contents($path);

if ($raw === false) {
    fwrite(STDERR, "error: unable to read OpenAPI file at {$path}\n");
    exit(1);
}

try {
    /** @var mixed $decoded */
    $decoded = json_decode($raw, true, 512, JSON_THROW_ON_ERROR);
} catch (JsonException $exception) {
    fwrite(STDERR, "error: invalid OpenAPI JSON: {$exception->getMessage()}\n");
    exit(1);
}

$normalize = function (mixed $value) use (&$normalize): mixed {
    if (! is_array($value)) {
        return $value;
    }

    $normalized = [];

    foreach ($value as $key => $child) {
        $normalized[$key] = $normalize($child);
    }

    if (! array_is_list($normalized)) {
        ksort($normalized, SORT_STRING);
    }

    return $normalized;
};

$normalizedDoc = $normalize($decoded);
$normalizedJson = json_encode($normalizedDoc, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);

if (! is_string($normalizedJson) || $normalizedJson === '') {
    fwrite(STDERR, "error: failed to normalize OpenAPI JSON\n");
    exit(1);
}

if (@file_put_contents($path, $normalizedJson) === false) {
    fwrite(STDERR, "error: unable to write normalized OpenAPI file at {$path}\n");
    exit(1);
}

fwrite(STDOUT, "ok: normalized OpenAPI at {$path}\n");
