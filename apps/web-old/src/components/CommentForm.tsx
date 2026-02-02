import { useMutation } from "convex/react";
import { useState } from "react";
import { api } from "../../convex/_generated/api";
import { withConvexProvider } from "../lib/convex.tsx";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Alert, AlertDescription } from "@/components/ui/alert";

export default withConvexProvider(function CommentForm() {
  const createComment = useMutation(api.comments.create);
  const [author, setAuthor] = useState("");
  const [content, setContent] = useState("");
  const [error, setError] = useState<string>();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!author.trim() || !content.trim()) {
      setError("You must provide an author and content");
      return;
    }

    setError(undefined);
    try {
      await createComment({ author, content });
      setAuthor("");
      setContent("");
    } catch {
      setError("Submission failed, try again.");
    }
  };

  return (
    <form onSubmit={handleSubmit} className="mb-8 space-y-4">
      <div className="space-y-2">
        <Label htmlFor="comment-author">Your name</Label>
        <Input
          id="comment-author"
          type="text"
          placeholder="Your name"
          value={author}
          onChange={(e) => setAuthor(e.target.value)}
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="comment-content">Comment</Label>
        <Textarea
          id="comment-content"
          placeholder="Leave a comment..."
          value={content}
          onChange={(e) => setContent(e.target.value)}
          className="min-h-[100px] resize-y"
        />
      </div>
      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      <Button type="submit" className="w-full md:w-auto">
        Post Comment
      </Button>
    </form>
  );
});
