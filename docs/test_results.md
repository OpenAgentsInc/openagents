  FAILED  Tests\Unit\BedrockMessageConverterTest > converts system message
  Failed asserting that two arrays are identical.
   Array &0 [
  -    'system' => 'You are a helpful assistant',
  -    'messages' => Array &1 [
  +    'system' => Array &1 [
           0 => Array &2 [
  +            'text' => 'You are a helpful assistant',
  +        ],
  +    ],
  +    'messages' => Array &3 [
  +        0 => Array &4 [
               'role' => 'user',
  -            'content' => Array &3 [
  -                0 => Array &4 [
  +            'content' => Array &5 [
  +                0 => Array &6 [
                       'text' => 'Hello',
                   ],
               ],


  at tests/Unit/BedrockMessageConverterTest.php:85
     81▕     ];
     82▕
     83▕     $result = $this->converter->convertToBedrockChatMessages($messages);
     84▕
  ➜  85▕     expect($result)->toBe([
     86▕         'system' => 'You are a helpful assistant',
     87▕         'messages' => [
     88▕             [
     89▕                 'role' => 'user',

  1   tests/Unit/BedrockMessageConverterTest.php:85

  ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   FAILED  Tests\Unit\BedrockMessageConverterTest > converts tool results
  Failed asserting that two arrays are identical.
               'role' => 'assistant',
               'content' => Array &6 [
                   0 => Array &7 [
  +                    'type' => 'text',
                       'text' => 'I'll help you with that.',
                   ],
                   1 => Array &8 [
  -                    'toolUse' => Array &9 [
  -                        'toolUseId' => 'tool123',
  -                        'name' => 'view_file',
  -                        'input' => Array &10 [
  -                            'path' => 'README.md',
  -                        ],
  +                    'type' => 'tool-call',
  +                    'toolCallId' => 'tool123',
  +                    'toolName' => 'view_file',
  +                    'args' => Array &9 [
  +                        'path' => 'README.md',
                       ],
                   ],
               ],
           ],
  -        2 => Array &11 [
  +        2 => Array &10 [
               'role' => 'user',
  -            'content' => Array &12 [
  -                0 => Array &13 [
  -                    'toolResult' => Array &14 [
  -                        'toolUseId' => 'tool123',
  -                        'content' => Array &15 [
  -                            0 => Array &16 [
  -                                'text' => '{"content":"README content here"}',
  -                            ],
  -                        ],
  -                    ],
  +            'content' => Array &11 [
  +                0 => Array &12 [
  +                    'text' => 'Continue.',
                   ],
               ],
           ],
       ],
   ]


  at tests/Unit/BedrockMessageConverterTest.php:151
    147▕     ];
    148▕
    149▕     $result = $this->converter->convertToBedrockChatMessages($messages);
    150▕
  ➜ 151▕     expect($result)->toBe([
    152▕         'system' => null,
    153▕         'messages' => [
    154▕             [
    155▕                 'role' => 'user',

  1   tests/Unit/BedrockMessageConverterTest.php:151
