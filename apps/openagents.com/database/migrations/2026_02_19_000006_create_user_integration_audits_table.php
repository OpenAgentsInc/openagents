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
        Schema::create('user_integration_audits', function (Blueprint $table): void {
            $table->id();
            $table->foreignId('user_id')->constrained()->cascadeOnDelete();
            $table->foreignId('user_integration_id')->nullable()->constrained('user_integrations')->nullOnDelete();
            $table->string('provider', 64);
            $table->string('action', 64);
            $table->json('metadata')->nullable();
            $table->timestamps();

            $table->index(['user_id', 'provider', 'created_at']);
            $table->index(['provider', 'action', 'created_at']);
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('user_integration_audits');
    }
};
