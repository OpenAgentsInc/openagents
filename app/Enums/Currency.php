<?php

namespace App\Enums;

enum Currency: string
{
    case BTC = 'btc';
    case USD = 'usd';
    case ECASH = 'ecash';
    case TAPROOT = 'taproot';
}
