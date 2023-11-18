<?php

namespace App\Console\Commands;

use App\Services\OpenAIGateway;
use GitHub;
use Illuminate\Console\Command;

class CommentOnIssue extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'comment {issuenum}';

    /**
     * The console command description.
     *
     * @var string
     */
    protected $description = 'Faerie will comment on a given GitHub issue';

    /**
     * Execute the console command.
     */
    public function handle()
    {
        $issueNum = $this->argument('issuenum');

        $response = GitHub::issues()->show('ArcadeLabsInc', 'openagents', $issueNum);

        $body = $response['body'];
        $title = $response['title'];

        $prompt = "You are Faerie, an AI agent specialized in writing & analyzing code.

  You have been summoned to ArcadeLabsInc/openagents issue #1.

  The issue is titled `" . $title . "`

  The issue body is:
  ```
  " . $body . "
  ```

  Please respond with the comment you would like to add to the issue. Write like a senior developer would write; don't introduce yourself or use flowery text or a closing signature.";

        $gateway = new OpenAIGateway();

        $response = $gateway->makeChatCompletion([
          'model' => 'gpt-4',
          'messages' => [
            // ['role' => 'system', 'content' => 'You are a helpful assistant.'],
            ['role' => 'user', 'content' => $prompt],
          ],
        ]);

        $comment = $response['choices'][0]['message']['content'];
        $this->info($comment);
        $this->info("POSTING...");

        GitHub::comments()->create('ArcadeLabsInc', 'openagents', $issueNum, array('body' => $comment));
        $this->info("DONE!");
    }
}
