<?php

test('smoke stream requires the secret header', function () {
    putenv('OA_SMOKE_SECRET=secret');
    $_ENV['OA_SMOKE_SECRET'] = 'secret';
    $_SERVER['OA_SMOKE_SECRET'] = 'secret';

    $this->get('/api/smoke/stream')->assertStatus(401);

    $this->get('/api/smoke/stream', ['x-oa-smoke-secret' => 'wrong'])->assertStatus(401);
});

test('smoke stream emits vercel-like sse frames when authorized', function () {
    putenv('OA_SMOKE_SECRET=secret');
    $_ENV['OA_SMOKE_SECRET'] = 'secret';
    $_SERVER['OA_SMOKE_SECRET'] = 'secret';

    $response = $this->get('/api/smoke/stream', ['x-oa-smoke-secret' => 'secret']);

    $response->assertOk();
    $response->assertHeader('x-oa-smoke', '1');

    $content = $response->streamedContent();

    expect($content)->toContain('data: {"type":"start"');
    expect($content)->toContain('data: {"type":"text-delta"');
    expect($content)->toContain('data: {"type":"finish"');
    expect($content)->toContain("data: [DONE]\n\n");
});
