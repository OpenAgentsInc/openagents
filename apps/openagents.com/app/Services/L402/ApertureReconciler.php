<?php

namespace App\Services\L402;

use App\Exceptions\L402\ApertureReconcileException;
use App\Models\L402Paywall;
use Illuminate\Support\Collection;
use RuntimeException;
use Symfony\Component\Process\Process;

class ApertureReconciler
{
    /**
     * @param  Collection<int, L402Paywall>  $paywalls
     * @return array<string, mixed>
     */
    public function reconcile(Collection $paywalls): array
    {
        $snapshot = $this->snapshotPayload($paywalls);
        $encoded = json_encode($snapshot, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES);

        if (! is_string($encoded)) {
            throw new RuntimeException('Failed to encode aperture paywall snapshot.');
        }

        $configPath = (string) config('lightning.operator.aperture_config_path', storage_path('app/l402/aperture-paywalls.json'));
        $this->writeSnapshot($configPath, $encoded);

        $command = trim((string) config('lightning.operator.aperture_reconcile_command', ''));
        $timeout = max(1, (int) config('lightning.operator.aperture_reconcile_timeout_seconds', 120));

        if ($command === '') {
            return [
                'status' => 'succeeded',
                'mode' => 'snapshot-only',
                'command' => null,
                'commandExecuted' => false,
                'stdout' => null,
                'stderr' => null,
                'configPath' => $configPath,
                'configSha256' => hash('sha256', $encoded),
                'activePaywallCount' => count($snapshot['services']),
                'snapshotVersion' => $snapshot['version'],
            ];
        }

        $process = Process::fromShellCommandline(
            $command,
            base_path(),
            [
                'OA_L402_APERTURE_CONFIG_FILE' => $configPath,
                'OA_L402_ACTIVE_PAYWALL_COUNT' => (string) count($snapshot['services']),
            ],
            null,
            $timeout,
        );

        $process->run();

        $stdout = trim($process->getOutput());
        $stderr = trim($process->getErrorOutput());

        if (! $process->isSuccessful()) {
            throw new ApertureReconcileException(
                'Aperture reconcile command failed.',
                [
                    'status' => 'failed',
                    'mode' => 'command',
                    'command' => $command,
                    'commandExecuted' => true,
                    'exitCode' => $process->getExitCode(),
                    'stdout' => $this->truncate($stdout),
                    'stderr' => $this->truncate($stderr),
                    'configPath' => $configPath,
                    'configSha256' => hash('sha256', $encoded),
                    'activePaywallCount' => count($snapshot['services']),
                    'snapshotVersion' => $snapshot['version'],
                ],
            );
        }

        return [
            'status' => 'succeeded',
            'mode' => 'command',
            'command' => $command,
            'commandExecuted' => true,
            'stdout' => $this->truncate($stdout),
            'stderr' => $this->truncate($stderr),
            'configPath' => $configPath,
            'configSha256' => hash('sha256', $encoded),
            'activePaywallCount' => count($snapshot['services']),
            'snapshotVersion' => $snapshot['version'],
        ];
    }

    /**
     * @param  Collection<int, L402Paywall>  $paywalls
     * @return array<string, mixed>
     */
    private function snapshotPayload(Collection $paywalls): array
    {
        return [
            'version' => now()->toISOString(),
            'services' => $paywalls
                ->filter(fn (L402Paywall $paywall) => ! $paywall->trashed() && $paywall->enabled)
                ->map(function (L402Paywall $paywall): array {
                    return [
                        'id' => (string) $paywall->id,
                        'name' => (string) $paywall->name,
                        'hostregexp' => (string) $paywall->host_regexp,
                        'pathregexp' => (string) $paywall->path_regexp,
                        'price' => (int) $paywall->price_msats,
                        'upstream' => (string) $paywall->upstream,
                        'enabled' => (bool) $paywall->enabled,
                        'meta' => is_array($paywall->meta) ? $paywall->meta : [],
                    ];
                })
                ->values()
                ->all(),
        ];
    }

    private function writeSnapshot(string $path, string $json): void
    {
        $directory = dirname($path);
        if (! is_dir($directory)) {
            mkdir($directory, 0755, true);
        }

        file_put_contents($path, $json);
    }

    private function truncate(?string $value, int $max = 4000): ?string
    {
        if (! is_string($value) || $value === '') {
            return null;
        }

        if (strlen($value) <= $max) {
            return $value;
        }

        return substr($value, 0, $max);
    }
}
