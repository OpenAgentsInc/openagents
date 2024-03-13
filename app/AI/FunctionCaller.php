<?php

namespace App\AI;

class FunctionCaller
{
    public static function parsedTools($tools)
    {
        $parsedTools = [];
        foreach ($tools as $tool) {
            if ($tool['type'] === 'function') {
                $parsedFunction = ['type' => $tool['type']];
                // Assuming your Function class has a method to export its properties as an array
                $parsedFunction['function'] = $tool['function'];
                $parsedTools[] = $parsedFunction;
            }
        }

        return $parsedTools;
    }

    public static function prepareFunctions()
    {
        return [
            [
                'type' => 'function',
                'function' => [
                    'name' => 'check_stock_price',
                    'description' => 'Check the current price of a stock given its ticker symbol',
                    'parameters' => [
                        'type' => 'object',
                        'properties' => [
                            'ticker_symbol' => [
                                'type' => 'string',
                                'description' => 'The stock ticker symbol.',
                            ],
                        ],
                        'required' => ['ticker_symbol'],
                    ],
                ],
            ],
            [
                'type' => 'function',
                'function' => [
                    'name' => 'company_news',
                    'description' => 'Retrieve news related to a company within a specified date range',
                    'parameters' => [
                        'type' => 'object',
                        'properties' => [
                            'symbol' => [
                                'type' => 'string',
                                'description' => 'Company symbol.',
                            ],
                            'from' => [
                                'type' => 'string',
                                'description' => 'From date YYYY-MM-DD.',
                            ],
                            'to' => [
                                'type' => 'string',
                                'description' => 'To date YYYY-MM-DD.',
                            ],
                        ],
                        'required' => ['symbol', 'from', 'to'],
                    ],
                ],
            ],

            [
                'type' => 'function',
                'function' => [
                    'name' => 'check_bitcoin_price',
                    'description' => 'Retrieve the bitcoin price',
                    'parameters' => [
                        'type' => 'object',
                        'properties' => [
                            'exchange' => [
                                'type' => 'string',
                                'description' => 'The exchange name.',
                            ],
                        ],
                        'required' => ['exchange'],
                    ],
                ],
            ],

        ];
    }
}
