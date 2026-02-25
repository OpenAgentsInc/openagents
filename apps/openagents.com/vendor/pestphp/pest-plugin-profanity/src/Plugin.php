<?php

declare(strict_types=1);

namespace Pest\Profanity;

use Pest\Contracts\Plugins\HandlesOriginalArguments;
use Pest\Plugins\Concerns\HandleArguments;
use Pest\Profanity\Contracts\Logger;
use Pest\Profanity\Logging\JsonLogger;
use Pest\Profanity\Logging\NullLogger;
use Pest\Profanity\Support\ConfigurationSourceDetector;
use Pest\TestSuite;
use Symfony\Component\Console\Output\OutputInterface;
use Symfony\Component\Finder\Finder;

use function Termwind\renderUsing;

/**
 * @internal
 */
class Plugin implements HandlesOriginalArguments
{
    use HandleArguments;

    /**
     * @var array<string>
     */
    private array $excludeWords = [];

    /**
     * @var array<string>
     */
    private array $includeWords = [];

    /**
     * @var array<string>|null
     */
    private $languages = null;

    private bool $compact = false;

    /**
     * The logger used to output profanity to a file.
     */
    private Logger $profanityLogger;

    /**
     * Creates a new Plugin instance.
     */
    public function __construct(
        private readonly OutputInterface $output
    ) {
        $this->profanityLogger = new NullLogger;
    }

    /**
     * {@inheritdoc}
     */
    public function handleOriginalArguments(array $arguments): void
    {
        if (! $this->hasArgument('--profanity', $arguments)) {
            return;
        }

        foreach ($arguments as $key => $argument) {
            if (str_starts_with($argument, '--exclude=')) {
                $words = explode(',', substr($argument, strlen('--exclude=')));
                $this->excludeWords = array_merge($this->excludeWords, $words);
                unset($arguments[$key]);
            }

            if (str_starts_with($argument, '--include=')) {
                $words = explode(',', substr($argument, strlen('--include=')));
                $this->includeWords = array_merge($this->includeWords, $words);
                unset($arguments[$key]);
            }

            if (str_starts_with($argument, '--language=')) {
                $languageValue = substr($argument, strlen('--language='));
                $this->languages = explode(',', $languageValue);
                $invalidLanguages = Validator::validateLanguages($this->languages);
                unset($arguments[$key]);
            }

            if (str_starts_with($argument, '--compact')) {
                $this->compact = true;
                unset($arguments[$key]);
            }

            if (str_starts_with($argument, '--output=')) {
                $outputPath = explode('=', $argument)[1] ?? null;

                if (empty($outputPath)) {
                    Output::errorMessage('No output path provided for [--profanity-json].');
                    $this->exit(1);
                }

                $this->profanityLogger = new JsonLogger(explode('=', $argument)[1]);
            }
        }

        if (! empty($invalidLanguages)) {
            $invalidLangsStr = implode(', ', $invalidLanguages);
            Output::errorMessage("The specified language does not exist: $invalidLangsStr");

            $this->output->writeln(['']);
            $this->output->writeln('<info>Available languages:</info>');

            $profanitiesDir = __DIR__.'/Config/profanities';
            $availableLanguages = array_map(
                fn ($file) => basename($file, '.php'),
                glob("$profanitiesDir/*.php")
            );

            $this->output->writeln(implode(', ', $availableLanguages));
            $this->output->writeln(['']);
            $this->exit(1);
        }

        $source = ConfigurationSourceDetector::detect();

        if ($source === []) {
            Output::errorMessage('No source section found. Did you forget to add a `source` section to your `phpunit.xml` file?');

            $this->exit(1);
        }

        $files = Finder::create()
            ->in($source)
            ->name('*.php')
            ->notPath('Config/profanities')
            ->notPath('src/Config/profanities')
            ->files();
        $filesWithProfanity = [];
        $totalProfanities = 0;

        $this->output->writeln(['']);

        Analyser::analyse(
            array_keys(iterator_to_array($files)),
            function (Result $result) use (&$filesWithProfanity, &$totalProfanities): void {
                $path = str_replace(TestSuite::getInstance()->rootPath.'/', '', $result->file);
                $errors = $result->errors;

                if (empty($errors)) {
                    if (! $this->compact) {
                        renderUsing($this->output);
                        Output::pass($path);

                        $this->profanityLogger->append($path, []);
                    }
                } else {
                    $filesWithProfanity[] = $path;
                    $totalProfanities += count($errors);

                    usort($errors, fn ($a, $b): int => $a->line <=> $b->line);

                    $profanityLines = [];
                    foreach ($errors as $error) {
                        $profanityLines[] = $error->getShortType().$error->line.'('.$error->word.')';
                    }

                    $this->profanityLogger->append($path, $profanityLines);

                    $profanityLines = implode(', ', $profanityLines);

                    renderUsing($this->output);
                    Output::fail($path, $profanityLines);
                }
            },
            $this->excludeWords,
            $this->includeWords,
            $this->languages
        );

        $filesWithProfanityCount = count($filesWithProfanity);
        $exitCode = (int) (! empty($filesWithProfanity));

        $this->profanityLogger->output();

        if ($exitCode === 1) {
            $instanceWord = $totalProfanities === 1 ? 'instance' : 'instances';
            $filesWord = $filesWithProfanityCount === 1 ? 'file' : 'files';
            Output::errorMessage("Found $totalProfanities $instanceWord of profanity in $filesWithProfanityCount $filesWord");
        } else {
            Output::successMessage('No profanity found in your application!');
        }

        $this->output->writeln(['']);
        $this->exit($exitCode);
    }

    /**
     * Exits the process with the given code.
     */
    public function exit(int $code): never
    {
        exit($code);
    }
}
