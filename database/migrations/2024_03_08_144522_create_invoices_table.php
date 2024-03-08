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
        Schema::create('invoices', function (Blueprint $table) {
            $table->id();
            $table->boolean('settled')->default(false);
            $table->string('state');
            $table->unsignedBigInteger('amount');
            $table->string('comment')->nullable();
            $table->timestamp('created_at_alby')->nullable();
            $table->string('creation_date')->nullable();
            $table->string('currency', 10);
            $table->text('destination_pubkey')->nullable();
            $table->unsignedBigInteger('expiry')->nullable();
            $table->string('fiat_currency', 10)->nullable();
            $table->integer('fiat_in_cents')->nullable();
            $table->string('identifier')->unique();
            $table->text('memo')->nullable();
            $table->string('metadata')->nullable();
            $table->string('payer_name')->nullable();
            $table->string('payer_email')->nullable();
            $table->string('payer_pubkey')->nullable();
            $table->text('payment_hash')->unique();
            $table->text('payment_request');
            $table->text('preimage')->nullable();
            $table->text('r_hash_str')->nullable();
            $table->string('type');
            $table->string('qr_code_png')->nullable();
            $table->string('qr_code_svg')->nullable();
            $table->unsignedBigInteger('value');
            $table->timestamp('settled_at')->nullable();
            $table->timestamp('expires_at')->nullable();
            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('invoices');
    }
};
