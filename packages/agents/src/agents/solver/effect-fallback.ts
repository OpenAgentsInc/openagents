/**
 * Fallback implementation for Effect AI in Cloudflare Workers
 * 
 * Since the Effect AI libraries have dependencies that are not compatible with
 * Cloudflare Workers, we provide a simple fallback implementation for demo purposes.
 */

// Define a type for the response to match what would come from Effect AI
export interface DadJokeResponse {
  text: string;
}

// List of dad jokes for the fallback implementation
const DAD_JOKES = [
  "Why don't scientists trust atoms? Because they make up everything!",
  "Did you hear about the mathematician who's afraid of negative numbers? He'll stop at nothing to avoid them!",
  "I told my wife she was drawing her eyebrows too high. She looked surprised.",
  "What do you call a fake noodle? An impasta!",
  "Why did the scarecrow win an award? Because he was outstanding in his field!",
  "I would tell you a joke about construction, but I'm still working on it.",
  "Why don't skeletons fight each other? They don't have the guts.",
  "I used to be a baker, but I couldn't make enough dough.",
  "How do you organize a space party? You planet!",
  "What's the best thing about Switzerland? I don't know, but the flag is a big plus.",
  "Did you hear about the claustrophobic astronaut? He just needed a little space.",
  "What do you call cheese that isn't yours? Nacho cheese!",
  "Why did the bicycle fall over? Because it was two tired!",
  "What did the ocean say to the beach? Nothing, it just waved.",
  "Why did the golfer bring two pairs of pants? In case he got a hole in one!"
];

/**
 * Generate a dad joke using a simple random selection
 * This is a fallback for when Effect AI doesn't work in Cloudflare Workers
 */
export async function generateDadJoke(prompt?: string): Promise<DadJokeResponse> {
  // Log that we're using the fallback implementation
  console.log("Using fallback implementation for dad joke generation");
  console.log("Original prompt:", prompt);
  
  // Select a random dad joke
  const randomIndex = Math.floor(Math.random() * DAD_JOKES.length);
  const joke = DAD_JOKES[randomIndex];
  
  // Add a note that this is a fallback
  return {
    text: `${joke}\n\n(Note: Using fallback implementation because Effect AI libraries aren't compatible with Cloudflare Workers)`
  };
}

// Export a fake namespace to match the Effect API shape
export const EffectAIFallback = {
  generateDadJoke
};