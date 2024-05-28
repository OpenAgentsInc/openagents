<?php

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\PaymentDestination;
use App\Models\PaymentSource;
use App\Models\User;

test('user can pay an agent', function () {
    $user = User::factory()->withBalance(1000 * 1000, Currency::BTC)->create();
    $agent = Agent::factory()->withBalance(0, Currency::BTC)->create();

    $user->payAgent($agent, 1000 * 1000, Currency::BTC);

    expect($user->checkBalance(Currency::BTC))->toBe(0)
        ->and($agent->checkBalance(Currency::BTC))->toBe(1000 * 1000)
        ->and($user->payments()->count())->toBe(1)
        ->and(PaymentSource::where('source_id', $user->id)->count())->toBe(1)
        ->and(PaymentDestination::where('destination_id', $agent->id)->count())->toBe(1);
});

test('user can pay a user', function () {
    $user = User::factory()->withBalance(1000 * 1000, Currency::BTC)->create();
    $user2 = User::factory()->withBalance(0, Currency::BTC)->create();

    $user->payUser($user2, 1000 * 1000, Currency::BTC);

    expect($user->checkBalance(Currency::BTC))->toBe(0)
        ->and($user2->checkBalance(Currency::BTC))->toBe(1000 * 1000)
        ->and($user->payments()->count())->toBe(1)
        ->and(PaymentSource::where('source_id', $user->id)->count())->toBe(1)
        ->and(PaymentDestination::where('destination_id', $user2->id)->count())->toBe(1);
});

test('agent can pay a user', function () {
    $agent = Agent::factory()->withBalance(1000 * 1000, Currency::BTC)->create();
    $user = User::factory()->withBalance(0, Currency::BTC)->create();

    $agent->payUser($user, 1000 * 1000, Currency::BTC);

    expect($agent->checkBalance(Currency::BTC))->toBe(0)
        ->and($user->checkBalance(Currency::BTC))->toBe(1000 * 1000)
        ->and($agent->payments()->count())->toBe(1)
        ->and(PaymentSource::where('source_id', $agent->id)->count())->toBe(1)
        ->and(PaymentDestination::where('destination_id', $user->id)->count())->toBe(1);
});

test('agent can pay an agent', function () {
    $agent = Agent::factory()->withBalance(1000 * 1000, Currency::BTC)->create();
    $agent2 = Agent::factory()->withBalance(0, Currency::BTC)->create();

    $agent->payAgent($agent2, 1000 * 1000, Currency::BTC);

    expect($agent->checkBalance(Currency::BTC))->toBe(0)
        ->and($agent2->checkBalance(Currency::BTC))->toBe(1000 * 1000)
        ->and($agent->payments()->count())->toBe(1)
        ->and(PaymentSource::where('source_id', $agent->id)->count())->toBe(1)
        ->and(PaymentDestination::where('destination_id', $agent2->id)->count())->toBe(1);
});

test('user can pay multipay users and agents', function () {
    $initialBalance = 100000000;
    $payEach = 1000000;

    $user = User::factory()->withBalance($initialBalance, Currency::BTC)->create();
    $recipientUsers = User::factory(20)->withBalance(0, Currency::BTC)->create();
    $recipientAgents = Agent::factory(5)->withBalance(0, Currency::BTC)->create();
    $recipients = $recipientUsers->concat($recipientAgents);

    $recipients->each(function ($recipient) use ($user, $payEach) {
        $user->multipay([
            [$recipient, $payEach, Currency::BTC],
        ]);
    });

    expect($user->checkBalance(Currency::BTC))->toBe($initialBalance - $payEach * 25)
        ->and($recipientUsers->fresh()->sum(fn ($recipient) => $recipient->checkBalance(Currency::BTC)))->toBe($payEach * 20)
        ->and(User::find(4)->fresh()->checkBalance(Currency::BTC))->toBe(1000000)
        ->and(Agent::find(3)->checkBalance(Currency::BTC))->toBe(1000000)
        ->and($user->payments()->count())->toBe(25)
        ->and(PaymentSource::where('source_type', get_class($user))->where('source_id', $user->id)->count())->toBe(25)
        ->and(PaymentDestination::whereIn('destination_id', $recipients->pluck('id'))->count())->toBe(25);
});

test('system can award balance increase', function () {
    $user = User::factory()->withBalance(1000 * 1000, Currency::BTC)->create();

    $user->payBonus(1000 * 1000, Currency::BTC, 'Bonus!');

    expect($user->checkBalance(Currency::BTC))->toBe(2000 * 1000)
        ->and($user->payments()->first()->description)->toBe('Bonus!')
        ->and(PaymentSource::where('source_id', $user->id)->count())->toBe(1)
        ->and(PaymentDestination::where('destination_id', $user->id)->count())->toBe(1);
});
