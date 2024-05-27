<?php

namespace App\Enums;

enum Currency: string
{
    case BTC = 'btc';
    case ECASH = 'ecash';
    case TAPROOT = 'taproot';
    case USD = 'usd';
}
