<?php

namespace App\Lightning\L402;

final class WwwAuthenticateParser
{
    public function parseL402Challenge(?string $wwwAuthenticateHeader): ?L402Challenge
    {
        if (! is_string($wwwAuthenticateHeader) || trim($wwwAuthenticateHeader) === '') {
            return null;
        }

        $pos = stripos($wwwAuthenticateHeader, 'l402');
        if ($pos === false) {
            return null;
        }

        $after = substr($wwwAuthenticateHeader, $pos + 4);

        $pairs = [];
        if (preg_match_all('/(\w+)="([^"]*)"/', $after, $m, PREG_SET_ORDER) !== false) {
            foreach ($m as $match) {
                $key = strtolower($match[1] ?? '');
                $val = $match[2] ?? '';
                if ($key !== '') {
                    $pairs[$key] = $val;
                }
            }
        }

        $macaroon = $pairs['macaroon'] ?? null;
        $invoice = $pairs['invoice'] ?? null;

        if (! is_string($macaroon) || $macaroon === '' || ! is_string($invoice) || $invoice === '') {
            return null;
        }

        return new L402Challenge($macaroon, $invoice);
    }
}
