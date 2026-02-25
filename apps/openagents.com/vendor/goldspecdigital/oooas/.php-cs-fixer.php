<?php

declare(strict_types=1);

use PhpCsFixer\Config;
use PhpCsFixer\Finder;

$finder = Finder::create()
    ->in(__DIR__)
    ->exclude([
        'media',
        'tests',
        'vendor',
    ]);

$rules = [
    '@PSR1' => true,
    '@PSR2' => true,
];

return (new Config())
    ->setRules($rules)
    ->setFinder($finder);
