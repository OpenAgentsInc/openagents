import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

export function MarkdownContent({ content }: { content: string }) {
  return (
    <ReactMarkdown remarkPlugins={[remarkGfm]} className="prose prose-neutral dark:prose-invert max-w-none">
      {content}
    </ReactMarkdown>
  );
}
