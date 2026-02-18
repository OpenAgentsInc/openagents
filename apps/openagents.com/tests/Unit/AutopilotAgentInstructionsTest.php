<?php

use App\AI\Agents\AutopilotAgent;
use App\AI\Runtime\AutopilotExecutionContext;

afterEach(function () {
    app()->forgetInstance(AutopilotExecutionContext::class);
});

test('autopilot instructions include runtime authenticated auth state', function () {
    $context = new AutopilotExecutionContext;
    $context->set(123, null, true);
    app()->instance(AutopilotExecutionContext::class, $context);

    $instructions = (string) (new AutopilotAgent)->instructions();

    expect($instructions)->toContain('Runtime session auth state (private): authenticated');
    expect($instructions)->toContain('If runtime auth state is authenticated: do not ask the user to sign in');
    expect($instructions)->toContain('/api/agent-payments/balance');
    expect($instructions)->toContain('/api/agent-payments/invoice');
    expect($instructions)->toContain('/api/shouts and json containing body');
    expect($instructions)->toContain('Never say an endpoint is unavailable unless discover has been run with limit=100');
});

test('autopilot instructions include runtime guest auth state', function () {
    $context = new AutopilotExecutionContext;
    $context->set(null, null, false);
    app()->instance(AutopilotExecutionContext::class, $context);

    $instructions = (string) (new AutopilotAgent)->instructions();

    expect($instructions)->toContain('Runtime session auth state (private): guest');
    expect($instructions)->toContain('If runtime auth state is guest: explain guest capabilities');
});
