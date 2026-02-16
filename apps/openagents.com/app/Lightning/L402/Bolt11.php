<?php

namespace App\Lightning\L402;

final class Bolt11
{
    /**
     * Parse the amount from a BOLT11 invoice string.
     *
     * Returns msats when the invoice includes an amount, otherwise null.
     *
     * This parser is intentionally minimal: for L402 cap enforcement we only need the amount.
     */
    public static function amountMsats(string $invoice): ?int
    {
        $invoice = strtolower(trim($invoice));

        // BOLT11 prefix is: ln + <currency> + <amount><multiplier_optional> + 1...
        // e.g. lnbc420n1...
        if (! preg_match('/^ln([a-z]{2})(\d+)?([munp])?1/', $invoice, $m)) {
            return null;
        }

        $digits = $m[2] ?? '';
        if ($digits === '') {
            return null;
        }

        // PHP ints are 64-bit on our targets; keep arithmetic bounded.
        $amount = (int) $digits;
        $mult = $m[3] ?? '';

        if ($mult === 'p') {
            // pico-BTC units correspond to 0.1 msat. Only accept integer msat values.
            if (($amount % 10) !== 0) {
                return null;
            }

            return intdiv($amount, 10);
        }

        $unitMsats = null;

        switch ($mult) {
            case '':
                $unitMsats = 100000000000;
                break;
            case 'm':
                $unitMsats = 100000000;
                break;
            case 'u':
                $unitMsats = 100000;
                break;
            case 'n':
                $unitMsats = 100;
                break;
            default:
                return null;
        }

        if ($amount > intdiv(PHP_INT_MAX, $unitMsats)) {
            return null;
        }

        return $amount * $unitMsats;
    }
}
