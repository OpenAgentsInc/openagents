<?php

use App\Models\Plugin;
use Extism\ExtismValType;
use Extism\HostFunction;

it('has a name', function () {
    $plugin = Plugin::factory()->create([
        'name' => 'Count Vowels',
    ]);
    expect($plugin->name)->toBe('Count Vowels');
});

it('has a description', function () {
    $plugin = Plugin::factory()->create([
        'description' => 'Count the vowels in a string',
    ]);
    expect($plugin->description)->toBe('Count the vowels in a string');
});

it('has a fee', function () {
    $plugin = Plugin::factory()->create([
        'fee' => 100,
    ]);
    expect($plugin->fee)->toBe(100);
});

it('has a wasm_url', function () {
    $plugin = Plugin::factory()->create([
        'wasm_url' => 'https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm',
    ]);
    expect($plugin->wasm_url)->toBe('https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm');
});

test('can execute plugin function', function () {
    $plugin = Plugin::factory()->create();
    $output = $plugin->call('count_vowels', 'Yellow, World!');
    expect($output)->toBe('{"count":3,"total":3,"vowels":"aeiouAEIOU"}');
})->group('integration');

test('can consume LLM inferencer JS plugin', function () {
    $plugin = Plugin::factory()->create([
        // 'wasm_url' => '/home/atlantispleb/code/plugin-llm-inferencer/plugin.wasm',
        'wasm_url' => 'https://github.com/OpenAgentsInc/plugin-llm-inferencer/releases/download/v0.0.0-alpha6/plugin.wasm',
    ]);

    // Define the HTTP fetch host function that returns a string
    $httpFetch = new HostFunction('httpFetch', [ExtismValType::I64], [ExtismValType::I64], function (string $requestJson) {
        // Decode the request JSON string
        //echo 'yo im here';

        return $requestJson;

        $request = json_decode($requestJson, true);

        // Initialize HTTP client
        $client = new \GuzzleHttp\Client();
        try {
            $response = $client->request($request['method'], $request['url'], [
                'headers' => $request['headers'],
                'body' => json_encode($request['body']),
                'http_errors' => false, // To ensure that the client returns a response object even on HTTP errors
            ]);

            // Prepare the response data
            $responseData = [
                'status' => $response->getStatusCode(),
                'headers' => $response->getHeaders(),
                'body' => (string) $response->getBody(),
            ];
        } catch (\Exception $e) {
            // Prepare error response data in case of an exception
            $responseData = [
                'status' => 500,
                'body' => $e->getMessage(),
            ];
        }

        // Convert the response data to a JSON string and return it
        return json_encode($responseData);
    });

    $plugin->initializePlugin([$httpFetch]);
    // $plugin->initializePlugin();

    // die and dump the length of the API key
    $input = json_encode([
        'apiKey' => env('OPENAI_API_KEY'),
        'hostUrl' => 'https://api.openai.com',
        // 'model' => 'gpt-3.5-turbo', // Or 'gpt-4', depending on the test case
        // 'messages' => [
        //     ['role' => 'system', 'content' => 'You are a helpful assistant.'],
        //     ['role' => 'user', 'content' => 'Hello!'],
        // ],
    ]);

    // escape the characters of input
    $escaped = addslashes($input);

    $output = $plugin->call('callOpenAI', 'what');
    dd($output);

    // Decode the JSON output to verify the expected structure and content
    $outputData = json_decode($output, true);
    dd($outputData);
    expect($outputData)->toHaveKey('choices');

    // You may adjust the validation of response content according to your requirements
    // For instance, to check for specific content in the assistant's response:
    // expect($outputData['choices'][0]['message']['content'])->toContain('expected response content');
});

test('can create a host function', function () {
    $hf = new HostFunction('test', [ExtismValType::I64], [ExtismValType::I64], function (string $a) {
        return $a;
    });
    expect($hf)->toBeInstanceOf(HostFunction::class);
})->group('integration');

test('PHP host functions work with the plugin', function () {
    // Set up a mock key-value store
    $kvstore = [];

    // Define the bytesToInt function
    function bytesToInt(string $bytes): int
    {
        $result = unpack('L', $bytes);

        return $result[1] ?? 0; // Ensure we return 0 if the unpack fails
    }

    // Define the kv_read host function
    $kvRead = new HostFunction('kv_read', [ExtismValType::I64], [ExtismValType::I64], function (string $key) use (&$kvstore) {
        $value = $kvstore[$key] ?? "\0\0\0\0";

        return $value;
    });

    // Define the kv_write host function
    $kvWrite = new HostFunction('kv_write', [ExtismValType::I64, ExtismValType::I64], [], function (string $key, string $value) use (&$kvstore) {
        $kvstore[$key] = $value;
    });

    // TODO: see if this is bad practice - model w constructor params instead of create
    // $plugin = new Plugin('https://github.com/extism/plugins/releases/latest/download/count_vowels_kvstore.wasm', [$kvRead, $kvWrite]);
    $plugin = Plugin::factory()->create([
        'wasm_url' => 'https://github.com/extism/plugins/releases/latest/download/count_vowels_kvstore.wasm',
    ]);
    $plugin->initializePlugin([$kvRead, $kvWrite]);

    // Call the plugin's function with the host functions in place
    $output = $plugin->call('count_vowels', 'Helloooo, World!');
    $output = $plugin->call('count_vowels', 'Hello, World!');

    // Assertions can vary based on the expected behavior; here's a basic check
    expect($output)->toBeJson();
    $decoded = json_decode($output, true);
    expect($decoded)->toHaveKey('total');
    expect($decoded['total'])->toEqual(9); // Assuming the kvstore increments
})->group('integration');

test('can return its module functions', function () {
    $plugin = Plugin::factory()->create([
        'name' => 'Count Vowels',
        'wasm_url' => 'https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm',
    ]);
    $functions = $plugin->functions();
    expect($functions)->toBeArray();
    expect($functions)->toContain('count_vowels');
})->group('integration');

it('can be parsed', function () {
    $plugin = Plugin::factory()->create();
    $parsed = $plugin->parse();
    expect($parsed)->toBeArray();
    expect($parsed['module_hash'])->toBeString();
    expect($parsed['module_hash'])->toBe('93898457953d30d016f712ccf4336ce7e9971db5f7f3aff1edd252764f75d5d7');
})->group('integration');
