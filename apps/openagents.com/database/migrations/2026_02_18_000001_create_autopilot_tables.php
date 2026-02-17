<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('autopilots', function (Blueprint $table) {
            $table->string('id', 36)->primary();
            $table->foreignId('owner_user_id')->index('autopilots_owner_user_id_index');
            $table->string('handle', 64)->unique('autopilots_handle_unique');
            $table->string('display_name', 120);
            $table->string('avatar', 255)->nullable();
            $table->string('status', 16)->default('active')->index('autopilots_status_index');
            $table->string('visibility', 16)->default('private');
            $table->string('tagline', 255)->nullable();
            $table->unsignedInteger('config_version')->default(1);
            $table->softDeletes();
            $table->timestamps();

            $table->index(['owner_user_id', 'updated_at'], 'autopilots_owner_updated_index');
            $table->index(['owner_user_id', 'visibility', 'updated_at'], 'autopilots_owner_visibility_updated_index');
        });

        Schema::create('autopilot_profiles', function (Blueprint $table) {
            $table->string('autopilot_id', 36)->primary();
            $table->string('owner_display_name', 120);
            $table->text('persona_summary')->nullable();
            $table->string('autopilot_voice', 64)->nullable();
            $table->json('principles')->nullable();
            $table->json('preferences')->nullable();
            $table->json('onboarding_answers')->nullable();
            $table->unsignedSmallInteger('schema_version')->default(1);
            $table->timestamps();
        });

        Schema::create('autopilot_policies', function (Blueprint $table) {
            $table->string('autopilot_id', 36)->primary();
            $table->string('model_provider', 64)->nullable();
            $table->string('model', 128)->nullable();
            $table->json('tool_allowlist')->nullable();
            $table->json('tool_denylist')->nullable();
            $table->boolean('l402_require_approval')->default(true);
            $table->unsignedBigInteger('l402_max_spend_msats_per_call')->nullable();
            $table->unsignedBigInteger('l402_max_spend_msats_per_day')->nullable();
            $table->json('l402_allowed_hosts')->nullable();
            $table->json('data_policy')->nullable();
            $table->timestamps();
        });

        Schema::create('autopilot_runtime_bindings', function (Blueprint $table) {
            $table->string('id', 36)->primary();
            $table->string('autopilot_id', 36)->index('autopilot_runtime_bindings_autopilot_id_index');
            $table->string('runtime_type', 32)->index('autopilot_runtime_bindings_runtime_type_index');
            $table->string('runtime_ref', 255)->nullable();
            $table->boolean('is_primary')->default(true);
            $table->timestamp('last_seen_at')->nullable();
            $table->json('meta')->nullable();
            $table->timestamps();

            $table->index(['autopilot_id', 'runtime_type'], 'autopilot_runtime_bindings_autopilot_runtime_type_index');
            $table->index(['autopilot_id', 'is_primary'], 'autopilot_runtime_bindings_autopilot_primary_index');
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('autopilot_runtime_bindings');
        Schema::dropIfExists('autopilot_policies');
        Schema::dropIfExists('autopilot_profiles');
        Schema::dropIfExists('autopilots');
    }
};
