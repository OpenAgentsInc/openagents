<?php

declare(strict_types=1);

$target = __DIR__.'/../vendor/goldspecdigital/oooas/src/Objects/BaseObject.php';

if (! file_exists($target)) {
    fwrite(STDOUT, "[patch-oooas] target not found, skipping\n");
    exit(0);
}

$contents = file_get_contents($target);
if ($contents === false) {
    fwrite(STDERR, "[patch-oooas] failed to read target file\n");
    exit(1);
}

$patched = str_replace(
    [
        'public function __construct(string $objectId = null)',
        'public static function create(string $objectId = null): self',
        'public static function ref(string $ref, string $objectId = null): self',
    ],
    [
        'public function __construct(?string $objectId = null)',
        'public static function create(?string $objectId = null): self',
        'public static function ref(string $ref, ?string $objectId = null): self',
    ],
    $contents,
);

if ($patched === $contents) {
    fwrite(STDOUT, "[patch-oooas] already patched\n");
    exit(0);
}

$result = file_put_contents($target, $patched);
if ($result === false) {
    fwrite(STDERR, "[patch-oooas] failed to write patched file\n");
    exit(1);
}

fwrite(STDOUT, "[patch-oooas] patched successfully\n");
exit(0);
