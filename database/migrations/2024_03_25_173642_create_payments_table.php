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
        Schema::create('payments', function (Blueprint $table) {
            $table->id();
            $table->timestamps();

            // Prism SinglePayment fields
            $table->uuid('prism_id')->unique(); // Same as "id" from prism API (not the "prism payment" id)
            $table->unsignedBigInteger('prism_created_at');
            $table->unsignedBigInteger('prism_updated_at');
            $table->unsignedBigInteger('expires_at');
            $table->string('receiver_id');
            $table->string('sender_id');
            $table->string('type');
            $table->bigInteger('amount_msat');
            $table->string('status');
            $table->boolean('resolved');
            $table->unsignedBigInteger('resolved_at')->nullable();
            $table->string('prism_payment_id')->nullable();
            $table->text('bolt11')->nullable();
            $table->text('preimage')->nullable();
            $table->string('failure_code')->nullable();

            // Indexes for faster searching on frequently queried fields
            $table->index(['sender_id', 'receiver_id', 'status']);
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('payments');
    }
};
