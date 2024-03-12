<?php

namespace App\Services;

use Exception;
use Finnhub\Api\DefaultApi;
use Finnhub\Configuration;
use Finnhub\Model\CompanyProfile2;
use Finnhub\Model\Quote;
use GuzzleHttp\Client;

class Finnhub
{
    private DefaultApi $client;

    public function __construct()
    {
        $config = Configuration::getDefaultConfiguration()->setApiKey('token', env('FINNHUB_API_KEY'));
        $this->client = new DefaultApi(
            new Client(),
            $config
        );
    }

    public function getStockCandles(string $symbol, string $resolution, int $from, int $to): array
    {
        try {
            $candles = $this->client->stockCandles($symbol, $resolution, $from, $to);

            return $candles;
        } catch (Exception $e) {
            // Handle the exception, e.g., log the error or throw a custom exception
            throw new Exception('Failed to retrieve stock candles: '.$e->getMessage());
        }
    }

    public function getCompanyProfile(string $symbol): CompanyProfile2
    {
        try {
            $profile = $this->client->companyProfile2($symbol);

            return $profile;
        } catch (Exception $e) {
            throw new Exception('Failed to retrieve company profile: '.$e->getMessage());
        }
    }

    public function getQuote(string $symbol): Quote
    {
        try {
            $quote = $this->client->quote($symbol);

            return $quote;
        } catch (Exception $e) {
            throw new Exception('Failed to retrieve stock quote: '.$e->getMessage());
        }
    }

    public function getCompanyNews(string $symbol, string $from, string $to): array
    {
        try {
            $news = $this->client->companyNews($symbol, $from, $to);

            return $news;
        } catch (Exception $e) {
            throw new Exception('Failed to retrieve company news: '.$e->getMessage());
        }
    }

    // Add more methods for other Finnhub API functions as needed
}
