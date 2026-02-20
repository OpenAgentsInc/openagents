<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('l402_control_plane_deployments', function (Blueprint $table) {
            $table->id();
            $table->string('deployment_id', 64)->unique();
            $table->string('paywall_id', 64)->nullable()->index();
            $table->string('owner_id', 64)->nullable()->index();
            $table->string('config_hash', 255)->index();
            $table->string('image_digest', 255)->nullable();
            $table->string('status', 32);
            $table->json('diagnostics')->nullable();
            $table->json('metadata')->nullable();
            $table->string('request_id', 255)->nullable()->index();
            $table->unsignedBigInteger('applied_at_ms')->nullable();
            $table->string('rolled_back_from', 64)->nullable();
            $table->timestamps();
        });

        Schema::create('l402_control_plane_gateway_events', function (Blueprint $table) {
            $table->id();
            $table->string('event_id', 64)->unique();
            $table->string('paywall_id', 64)->index();
            $table->string('owner_id', 64)->index();
            $table->string('event_type', 120);
            $table->string('level', 16);
            $table->string('request_id', 255)->nullable()->index();
            $table->json('metadata')->nullable();
            $table->timestamps();
        });

        Schema::create('l402_control_plane_invoices', function (Blueprint $table) {
            $table->id();
            $table->string('invoice_id', 64)->unique();
            $table->string('paywall_id', 64)->index();
            $table->string('owner_id', 64)->index();
            $table->unsignedBigInteger('amount_msats');
            $table->string('status', 32);
            $table->text('payment_hash')->nullable();
            $table->text('payment_request')->nullable();
            $table->text('payment_proof_ref')->nullable();
            $table->string('request_id', 255)->nullable();
            $table->unsignedBigInteger('settled_at_ms')->nullable();
            $table->timestamps();
        });

        Schema::create('l402_control_plane_settlements', function (Blueprint $table) {
            $table->id();
            $table->string('settlement_id', 64)->unique();
            $table->string('paywall_id', 64)->index();
            $table->string('owner_id', 64)->index();
            $table->string('invoice_id', 64)->nullable()->index();
            $table->unsignedBigInteger('amount_msats');
            $table->string('payment_proof_ref', 255);
            $table->string('request_id', 255)->nullable();
            $table->json('metadata')->nullable();
            $table->timestamps();
        });

        Schema::create('l402_control_plane_security_global', function (Blueprint $table) {
            $table->string('state_id', 64)->primary();
            $table->boolean('global_pause')->default(false);
            $table->string('deny_reason_code', 64)->nullable();
            $table->text('deny_reason')->nullable();
            $table->string('updated_by', 255)->nullable();
            $table->unsignedBigInteger('updated_at_ms')->nullable();
            $table->timestamps();
        });

        Schema::create('l402_control_plane_owner_controls', function (Blueprint $table) {
            $table->string('owner_id', 64)->primary();
            $table->boolean('kill_switch')->default(false);
            $table->string('deny_reason_code', 64)->nullable();
            $table->text('deny_reason')->nullable();
            $table->string('updated_by', 255)->nullable();
            $table->unsignedBigInteger('updated_at_ms')->nullable();
            $table->timestamps();
        });

        Schema::create('l402_control_plane_credential_roles', function (Blueprint $table) {
            $table->string('role', 64)->primary();
            $table->string('status', 32);
            $table->unsignedInteger('version')->default(1);
            $table->string('fingerprint', 255)->nullable();
            $table->text('note')->nullable();
            $table->unsignedBigInteger('updated_at_ms')->nullable();
            $table->unsignedBigInteger('last_rotated_at_ms')->nullable();
            $table->unsignedBigInteger('revoked_at_ms')->nullable();
            $table->timestamps();
        });

        DB::table('l402_control_plane_security_global')->insert([
            'state_id' => 'global',
            'global_pause' => false,
            'deny_reason_code' => null,
            'deny_reason' => null,
            'updated_by' => null,
            'updated_at_ms' => 0,
            'created_at' => now(),
            'updated_at' => now(),
        ]);
    }

    public function down(): void
    {
        Schema::dropIfExists('l402_control_plane_credential_roles');
        Schema::dropIfExists('l402_control_plane_owner_controls');
        Schema::dropIfExists('l402_control_plane_security_global');
        Schema::dropIfExists('l402_control_plane_settlements');
        Schema::dropIfExists('l402_control_plane_invoices');
        Schema::dropIfExists('l402_control_plane_gateway_events');
        Schema::dropIfExists('l402_control_plane_deployments');
    }
};
