<?php

namespace App\Console\Commands;

use App\Services\OpenAIGateway;
use App\Services\Patcher;
use App\Services\Planner;
use App\Services\Searcher;
use GitHub;
use Illuminate\Console\Command;

class CodeIssue extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'code {issuenum}';

    /**
     * The console command description.
     *
     * @var string
     */
    protected $description = 'Faerie will write code for a given GitHub issue';

    /**
     * Execute the console command.
     */
    public function handle()
    {
        // Grab the issue number from the command line
        $issueNum = $this->argument('issuenum');

        // Grab the issue body and comments from GitHub
        $response = GitHub::issues()->show('ArcadeLabsInc', 'openagents', $issueNum);
        $commentsResponse = GitHub::api('issue')->comments()->all('ArcadeLabsInc', 'openagents', 1);
        $body = $response['body'];
        $title = $response['title'];

        $patcher = new Patcher();
        $planner = new Planner();

        // Format the issue and comments as messages
        $userAndAssistantMessages = $planner->formatIssueAndCommentsAsMessages($body, $commentsResponse);

        // Build the context from a summary of the messages passed as query to a similarity search
        $context = $this->buildContextFrom($userAndAssistantMessages);

        // Build the prompt from the context and the issue title
        $commitPrompt = "You are Faerie, an AI agent specialized in writing & analyzing code.

  You have been summoned to solve an issue titled `" . $title . "`

  You cannot speak English, but you can write code. You respond only with code.

  The issue body is the first message below.

  For additional context, consult the following code snippets:
  ---
  " . $context . "
  ---

  Remember, respond ONLY with a unified code diff. Do not include any other text. Do not use natural language. Only code.";

        $systemMessages = [
            ['role' => 'system', 'content' => $commitPrompt],
        ];
        $messages = array_merge($systemMessages, $userAndAssistantMessages,
        [
            ['role' => 'user', 'content' => 'Please respond ONLY with a unified code diff we can submit directly to GitHub. Do not include any other text. Do not use natural language. Only the code diff.'],
        ]);

        $gateway = new OpenAIGateway();
        $response = $gateway->makeChatCompletion([
          'model' => 'gpt-4',
          'messages' => $messages,
        ]);
        $commit = $response['choices'][0]['message']['content'];
        $this->info($commit);




        dd();

        // Build the prompt from the context and the issue title
        $commentPrompt = "You are Faerie, an AI agent specialized in writing & analyzing code.

  You have been summoned to ArcadeLabsInc/openagents issue #1.

  The issue is titled `" . $title . "`

  The issue body is the first message below.

  For additional context, consult the following code snippets:
  ---
  " . $context . "
  ---

  Please respond with the comment you would like to add to the issue. Write like a senior developer would write; don't introduce yourself or use flowery text or a closing signature.";

        // Build the messages array
        $systemMessages = [
          ['role' => 'system', 'content' => $commentPrompt],
        ];
        $messages = array_merge($systemMessages, $userAndAssistantMessages);

        // Make the chat completion to generate the comment
        $response = $gateway->makeChatCompletion([
          'model' => 'gpt-4',
          'messages' => $messages,
        ]);
        $comment = $response['choices'][0]['message']['content'];
        $this->info($comment);

        // Post the comment to GitHub
        $this->info("POSTING...");
        GitHub::api('issue')->comments()->create('ArcadeLabsInc', 'openagents', $issueNum, array('body' => $comment));
        $this->info("DONE!");
    }

    private function buildContextFrom(array $messages): string
    {
        $queryInput = '';

        // todo: add author to each message?
        foreach ($messages as $message) {
            $queryInput .= $message['content'] . "\n---\n";
        }

        // Let's do one LLM call to summarize queryInput with an emphasis on the types of files we need to look up
        $gateway = new OpenAIGateway();
        $response = $gateway->makeChatCompletion([
          'model' => 'gpt-4',
          'messages' => [
            ['role' => 'system', 'content' => 'You are a helpful assistant. Speak concisely. Answer the user\'s question based only on the following context: ' . $queryInput],
            ['role' => 'user', 'content' => 'Write 2-3 sentences explaining the types of files we should search for in our codebase to provide the next response in the conversation. Focus only on the next step, not future optimizations. Ignore mentions of video transcriptions or readme/documentation.'],
          ],
        ]);
        $query = $response['choices'][0]['message']['content'];

        $searcher = new Searcher();
        $results = $searcher->queryAllFiles($query);

        // Loop through the results, and for each one, add the file name and the entire content of the file to the context
        $context = '';
        foreach ($results["results"] as $result) {
            $context .= "Content of " . $result['path'] . ": \n```\n";
            $content = $this->getFileContent($result['path']);
            $context .= $content . "\n```\n\n";
        }

        return $context;
    }

    private function getFileContent(string $path): string
    {
        $file = fopen($path, 'r');
        $content = fread($file, filesize($path));
        fclose($file);

        return $content;
    }
}
