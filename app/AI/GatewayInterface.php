<?php
declare(strict_types=1);

namespace App\AI;

interface GatewayInterface
{
    public function inference(array $params);
}
