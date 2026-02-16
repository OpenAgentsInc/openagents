<?php

$emails = array_values(array_filter(array_map(
    static fn (string $email): string => strtolower(trim($email)),
    explode(',', (string) env('ADMIN_EMAILS', 'chris@openagents.com')),
), static fn (string $email): bool => $email !== ''));

return [
    'emails' => array_values(array_unique($emails)),
];
