<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('l402_paywalls', function (Blueprint $table) {
            $table->string('id', 36)->primary();
            $table->foreignId('owner_user_id')->index('l402_paywalls_owner_user_id_index');

            $table->string('name', 120);
            $table->string('host_regexp', 255);
            $table->string('path_regexp', 255);
            $table->unsignedBigInteger('price_msats');
            $table->string('upstream', 2048);
            $table->boolean('enabled')->default(true);

            $table->json('meta')->nullable();

            $table->string('last_reconcile_status', 32)->nullable();
            $table->text('last_reconcile_error')->nullable();
            $table->timestamp('last_reconciled_at')->nullable();

            $table->softDeletes();
            $table->timestamps();

            $table->index(['enabled', 'updated_at'], 'l402_paywalls_enabled_updated_index');
            $table->index(['owner_user_id', 'updated_at'], 'l402_paywalls_owner_updated_index');
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('l402_paywalls');
    }
};
