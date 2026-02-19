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
        Schema::create('comms_delivery_projections', function (Blueprint $table) {
            $table->id();
            $table->foreignId('user_id')->constrained()->cascadeOnDelete();
            $table->string('provider', 32);
            $table->string('integration_id', 191)->default('unknown');
            $table->string('last_state', 32)->nullable();
            $table->timestamp('last_event_at')->nullable();
            $table->string('last_message_id', 191)->nullable();
            $table->string('last_recipient', 191)->nullable();
            $table->string('runtime_event_id', 191)->nullable();
            $table->string('source', 64)->default('runtime_forwarder');
            $table->foreignId('last_webhook_event_id')->nullable()->constrained('comms_webhook_events')->nullOnDelete();
            $table->timestamps();

            $table->unique(['user_id', 'provider', 'integration_id'], 'comms_delivery_proj_unique_scope');
            $table->index(['provider', 'last_state'], 'comms_delivery_proj_provider_state_idx');
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('comms_delivery_projections');
    }
};
