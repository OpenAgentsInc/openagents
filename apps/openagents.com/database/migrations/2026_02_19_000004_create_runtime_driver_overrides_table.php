<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('runtime_driver_overrides', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->string('scope_type', 16); // user|autopilot
            $table->string('scope_id', 64);
            $table->string('driver', 16); // legacy|elixir
            $table->boolean('is_active')->default(true);
            $table->string('reason', 255)->nullable();
            $table->json('meta')->nullable();
            $table->timestamps();

            $table->unique(['scope_type', 'scope_id'], 'runtime_driver_overrides_scope_unique');
            $table->index(['is_active', 'scope_type', 'scope_id'], 'runtime_driver_overrides_active_scope_index');
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('runtime_driver_overrides');
    }
};
