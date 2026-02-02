import { useQuery } from "convex/react";
import { api } from "../../convex/_generated/api";
import { withConvexProvider } from "../lib/convex.tsx";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

export default withConvexProvider(function CommentList() {
  const comments = useQuery(api.comments.list);

  if (comments === undefined) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-24 w-full" />
        <Skeleton className="h-24 w-full" />
        <Skeleton className="h-24 w-full" />
      </div>
    );
  }

  if (comments.length === 0) {
    return (
      <p className="py-4 text-center text-muted-foreground">
        No comments found.
      </p>
    );
  }

  return (
    <div className="space-y-4">
      {comments.map((comment) => (
        <Card key={comment._id}>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-base font-medium">
              {comment.author}
            </CardTitle>
            <span className="text-sm text-muted-foreground">
              {new Date(comment._creationTime).toLocaleDateString()}
            </span>
          </CardHeader>
          <CardContent>
            <p className="whitespace-pre-line text-sm text-muted-foreground">
              {comment.content}
            </p>
          </CardContent>
        </Card>
      ))}
    </div>
  );
});
