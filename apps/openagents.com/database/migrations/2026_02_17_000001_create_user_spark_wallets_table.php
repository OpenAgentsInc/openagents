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
        Schema::create('user_spark_wallets', function (Blueprint $table) {
            $table->id();
            $table->foreignId('user_id')->constrained()->cascadeOnDelete()->unique();
            $table->string('wallet_id', 128)->unique();
            $table->text('mnemonic');
            $table->string('spark_address', 255)->nullable();
            $table->string('lightning_address', 255)->nullable();
            $table->string('identity_pubkey', 255)->nullable();
            $table->unsignedBigInteger('last_balance_sats')->nullable();
            $table->string('status', 32)->default('active');
            $table->string('provider', 64)->default('spark_executor');
            $table->text('last_error')->nullable();
            $table->json('meta')->nullable();
            $table->timestamp('last_synced_at')->nullable();
            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('user_spark_wallets');
    }
};
