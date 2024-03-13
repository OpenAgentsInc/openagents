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
                    'name' => 'retrieve_payment_status',
                    'description' => 'Get payment status of a transaction',
                    'parameters' => [
                        'type' => 'object',
                        'properties' => [
                            'transaction_id' => [
                                'type' => 'string',
                                'description' => 'The transaction id.',
                            ],
                        ],
                        'required' => ['transaction_id'],
                    ],
                ],
            ],
            [
                'type' => 'function',
                'function' => [
                    'name' => 'retrieve_payment_date',
                    'description' => 'Get payment date of a transaction',
                    'parameters' => [
                        'type' => 'object',
                        'properties' => [
                            'transaction_id' => [
                                'type' => 'string',
                                'description' => 'The transaction id.',
                            ],
                        ],
                        'required' => ['transaction_id'],
                    ],
                ],
            ],
        ];
    }
}
