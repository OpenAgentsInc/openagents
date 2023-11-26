<?php

namespace App\Console\Commands;

// use App\Services\OpenAIGateway;
// use App\Services\Patcher;
// use App\Services\Planner;
// use App\Services\Searcher;
// use GitHub;
use Illuminate\Console\Command;

class CodeAndComment extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'codenow {issuenum}';

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
          dd("TEST");
  //       // Grab the issue number from the command line
  //       $issueNum = $this->argument('issuenum');

  //       // Grab the issue body and comments from GitHub
  //       $response = GitHub::issues()->show('ArcadeLabsInc', 'openagents', $issueNum);
  //       $commentsResponse = GitHub::api('issue')->comments()->all('ArcadeLabsInc', 'openagents', 1);
  //       $body = $response['body'];
  //       $title = $response['title'];

  //       $patcher = new Patcher();
  //       $planner = new Planner();

  //       // Format the issue and comments as messages
  //       $userAndAssistantMessages = $planner->formatIssueAndCommentsAsMessages($body, $commentsResponse);

  //       // Build the context from a summary of the messages passed as query to a similarity search
  //       $context = $this->buildContextFrom($userAndAssistantMessages);

  //       $taskDescription = $planner->createPlan($userAndAssistantMessages);

  //       $planPrompt = "A description of your next task is:" . $taskDescription . "

  // For additional context, consult the following code snippets:
  // ---
  // " . $context;

  //       $patches = $patcher->getIssuePatches([
  //           "title" => $title,
  //           "body" => $planPrompt
  //       ]);

  //       print_r($patches);
  //       print_r("---");

  //       $res = $patcher->submitPatchesToGitHub($patches, "ArcadeLabsInc/trashheap", "testbranch");
  //       print_r($res);

  //       $this->info("Done!");
    }

    // private function buildContextFrom(array $messages): string
    // {
    //     $queryInput = '';

    //     // todo: add author to each message?
    //     foreach ($messages as $message) {
    //         $queryInput .= $message['content'] . "\n---\n";
    //     }

    //     // Let's do one LLM call to summarize queryInput with an emphasis on the types of files we need to look up
    //     $gateway = new OpenAIGateway();
    //     $response = $gateway->makeChatCompletion([
    //       'model' => 'gpt-4',
    //       'messages' => [
    //         ['role' => 'system', 'content' => 'You are a helpful assistant. Speak concisely. Answer the user\'s question based only on the following context: ' . $queryInput],
    //         ['role' => 'user', 'content' => 'Write 2-3 sentences explaining the types of files we should search for in our codebase to provide the next response in the conversation. Focus only on the next step, not future optimizations. Ignore mentions of video transcriptions or readme/documentation.'],
    //       ],
    //     ]);
    //     $query = $response['choices'][0]['message']['content'];

    //     $searcher = new Searcher();
    //     $results = $searcher->queryAllFiles($query);

    //     // Loop through the results, and for each one, add the file name and the entire content of the file to the context
    //     $context = '';
    //     foreach ($results["results"] as $result) {
    //         $context .= "Content of " . $result['path'] . ": \n```\n";
    //         $content = $this->getFileContent($result['path']);
    //         $context .= $content . "\n```\n\n";
    //     }

    //     return $context;
    // }

    // private function getFileContent(string $path): string
    // {
    //     $file = fopen($path, 'r');
    //     $content = fread($file, filesize($path));
    //     fclose($file);

    //     return $content;
    // }
}
