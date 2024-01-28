<?php

namespace App\Services;

use App\Services\Alby\Client as AlbyClient;
use Illuminate\Support\Facades\Http;

class L402
{
    private $albyClient;

    public function __construct()
    {
        $this->albyClient = new AlbyClient(env('ALBY_ACCESS_TOKEN'));
    }

    public function handleL402Request($url)
    {
        $response = $this->sendRequest($url);

        if ($response->status() == 402) {
            $authHeader = $response->header('WWW-Authenticate');
            [$macaroon, $invoiceString] = $this->parseAuthHeader($authHeader);

            $payment = $this->albyClient->payInvoice($invoiceString);
            $preimage = $payment['payment_preimage'];

            // Assemble the L402 token
            $l402Token = $this->assembleL402Token($macaroon, $preimage);

            // Retry the request with the L402 token
            $response = $this->sendRequest($url, $l402Token);
        }

        // Process the response as needed
        return $response;
    }

    private function sendRequest($url, $l402Token = null)
    {
        $headers = [];
        if ($l402Token) {
            $headers['Authorization'] = 'L402 '.$l402Token;
        }

        return Http::withHeaders($headers)->get($url);
    }

    private function parseAuthHeader($authHeader)
    {
        if (str_starts_with($authHeader, 'L402 ')) {
            $authHeader = substr($authHeader, 5); // Remove 'L402 ' prefix
        }

        $pattern = '/macaroon="([^"]+)", invoice="([^"]+)"/';
        if (preg_match($pattern, $authHeader, $matches) && count($matches) === 3) {
            return [$matches[1], $matches[2]]; // Extracted macaroon and invoice
        }

        throw new \Exception("Invalid 'WWW-Authenticate' header format");
    }

    private function assembleL402Token($macaroon, $preimage)
    {
        return $macaroon.':'.$preimage;
    }
}
