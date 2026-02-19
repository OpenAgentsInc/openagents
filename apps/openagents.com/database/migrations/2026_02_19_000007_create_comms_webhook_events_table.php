<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::create('comms_webhook_events', function (Blueprint $table) {
            $table->id();
            $table->string('provider', 32);
            $table->string('idempotency_key', 191)->unique();
            $table->string('external_event_id', 191)->nullable();
            $table->string('event_type', 64)->nullable();
            $table->string('delivery_state', 32)->nullable();
            $table->string('message_id', 191)->nullable();
            $table->string('integration_id', 191)->nullable();
            $table->unsignedBigInteger('user_id')->nullable();
            $table->string('recipient', 191)->nullable();
            $table->boolean('signature_valid')->default(false);
            $table->string('status', 32)->default('received');
            $table->string('normalized_hash', 64)->nullable();
            $table->unsignedInteger('runtime_attempts')->default(0);
            $table->unsignedSmallInteger('runtime_status_code')->nullable();
            $table->json('runtime_response')->nullable();
            $table->json('normalized_payload')->nullable();
            $table->json('raw_payload')->nullable();
            $table->text('last_error')->nullable();
            $table->timestamp('forwarded_at')->nullable();
            $table->timestamps();

            $table->index(['provider', 'external_event_id'], 'comms_webhook_events_provider_external_idx');
            $table->index(['provider', 'status'], 'comms_webhook_events_provider_status_idx');
            $table->index(['provider', 'integration_id'], 'comms_webhook_events_provider_integration_idx');
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('comms_webhook_events');
    }
};
