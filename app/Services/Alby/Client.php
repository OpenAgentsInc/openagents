<?php

namespace App\Services\Alby;

use App\Services\Alby\Contracts\AlbyClient;
use GuzzleHttp;
use Illuminate\Support\Facades\Http;

class Client implements AlbyClient
{
    private $client;

    private $access_token;

    private $refresh_token;

    public function __construct($access_token)
    {
        $this->url = 'https://api.getalby.com';
        $this->access_token = $access_token;
    }

    // deprecated
    public function init()
    {
        return true;
    }

    private function request($method, $path, $body = null)
    {
        $headers = [
            'Accept' => 'application/json',
            'Content-Type' => 'application/json',
            'Access-Control-Allow-Origin' => '*',
            'Authorization' => "Bearer {$this->access_token}",
            'User-Agent' => 'alby-php',
        ];

        $requestBody = $body ? json_encode($body) : null;
        $request = new GuzzleHttp\Psr7\Request(
            $method,
            $path,
            $headers,
            $requestBody
        );
        try {
            $response = $this->client()->send($request);
            $responseBody = $response->getBody()->getContents();

            return json_decode($responseBody, true);
        } catch (GuzzleHttp\Exception\ClientException $e) {
            dd($e);
            $error = json_decode($e->getResponse()->getBody()->getContents(), true);
            throw new \Exception($error['error']);
        }
    }

    public function getInfo(): array
    {
        $data = $this->request('GET', '/user/me');
        $data['alias'] = '🐝 getalby.com';
        $data['identity_pubkey'] = '';

        return $data;
    }

    public function getBalance()
    {
        $data = $this->request('GET', '/balance');

        return $data;
    }

    private function client()
    {
        if ($this->client) {
            return $this->client;
        }
        $options = ['base_uri' => $this->url, 'timeout' => 10];
        $this->client = new GuzzleHttp\Client($options);

        return $this->client;
    }

    public function isConnectionValid(): bool
    {
        return ! empty($this->access_token);
    }

    public function addInvoice($invoice): array
    {
        $params = ['amount' => $invoice['value'], 'memo' => $invoice['memo']];
        if (array_key_exists('description_hash', $invoice) && ! empty($invoice['description_hash'])) {
            $params['description_hash'] = $invoice['description_hash'];
        }
        if (array_key_exists('unhashed_description', $invoice) && ! empty($invoice['unhashed_description'])) {
            $params['description'] = $invoice['unhashed_description'];
        }
        $data = $this->request('POST', '/invoices', $params);
        $data['id'] = $data['payment_hash'];
        $data['r_hash'] = $data['payment_hash'];

        return $data;
    }

    public function getInvoice($rHash): array
    {
        $invoice = $this->request('GET', "/invoices/{$rHash}");

        return $invoice;
    }

    public function decodeInvoice($bolt11): array
    {
        $invoice = $this->request('GET', "/decode/bolt11/{$bolt11}");

        return $invoice;
    }

    public function payInvoice($bolt11): array
    {
        $payment = $this->request('POST', '/payments/bolt11', ['invoice' => $bolt11]);

        return $payment;
    }

    public function isInvoicePaid($rHash): bool
    {
        $invoice = $this->getInvoice($rHash);

        return $invoice['settled'];
    }

    public function requestInvoiceForLightningAddress($data)
    {
        $lightningAddress = $data['lightning_address'];
        $amount = $data['amount'];
        $memo = $data['memo'] ?? 'OpenAgents Withdrawal';

        $url = 'https://api.getalby.com/lnurl/generate-invoice?ln='.urlencode($lightningAddress).'&amount='.$amount.'&comment='.urlencode($memo);
        $invoice = Http::get($url)->json();

        return $invoice['invoice'];
    }
}
