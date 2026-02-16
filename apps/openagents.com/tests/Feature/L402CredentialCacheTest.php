<?php

use App\Lightning\L402\L402CredentialCache;
use App\Models\L402Credential;

beforeEach(function () {
    config()->set('lightning.l402.allowlist_hosts', ['fake-l402.local']);
});

test('credential cache round-trips values and expiry', function () {
    $cache = resolve(L402CredentialCache::class);

    $cache->put(
        host: 'fake-l402.local',
        scope: 'demo.fake',
        macaroon: 'macaroon_abc',
        preimage: str_repeat('a', 64),
        ttlSeconds: 60,
    );

    $v = $cache->get('fake-l402.local', 'demo.fake');

    expect($v)->not->toBeNull();
    expect($v->macaroon)->toBe('macaroon_abc');
    expect($v->preimage)->toBe(str_repeat('a', 64));
    expect($v->expiresAt)->not->toBeNull();
    expect($v->expiresAt->isFuture())->toBeTrue();

    expect(L402Credential::query()->count())->toBe(1);
});

test('expired credentials are deleted on read', function () {
    L402Credential::query()->create([
        'host' => 'fake-l402.local',
        'scope' => 'demo.fake',
        'macaroon' => 'macaroon_abc',
        'preimage' => str_repeat('b', 64),
        'expires_at' => now()->subSecond(),
    ]);

    $cache = resolve(L402CredentialCache::class);

    expect($cache->get('fake-l402.local', 'demo.fake'))->toBeNull();
    expect(L402Credential::query()->count())->toBe(0);
});

test('delete removes credentials', function () {
    $cache = resolve(L402CredentialCache::class);

    $cache->put(
        host: 'fake-l402.local',
        scope: 'demo.fake',
        macaroon: 'macaroon_abc',
        preimage: str_repeat('c', 64),
        ttlSeconds: 60,
    );

    expect(L402Credential::query()->count())->toBe(1);

    $cache->delete('fake-l402.local', 'demo.fake');

    expect(L402Credential::query()->count())->toBe(0);
    expect($cache->get('fake-l402.local', 'demo.fake'))->toBeNull();
});
