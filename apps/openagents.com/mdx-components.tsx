import type { MDXComponents } from 'mdx/types';
import { Text } from '@arwes/react';

export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    h1: (props: any) => <Text as="h1" className="text-3xl font-bold text-cyan-100 mt-8 mb-4" {...props} />,
    h2: (props: any) => <Text as="h2" className="text-2xl font-semibold text-cyan-100 mt-6 mb-3" {...props} />,
    h3: (props: any) => <Text as="h3" className="text-xl font-semibold text-cyan-200 mt-4 mb-2" {...props} />,
    p: (props: any) => <Text className="text-cyan-300/80 mb-4 leading-relaxed" {...props} />,
    a: (props: any) => <a className="text-cyan-400 hover:text-cyan-300 underline transition-colors" {...props} />,
    ul: (props: any) => <ul className="list-disc list-inside text-cyan-300/80 mb-4 space-y-1" {...props} />,
    ol: (props: any) => <ol className="list-decimal list-inside text-cyan-300/80 mb-4 space-y-1" {...props} />,
    li: (props: any) => <li className="text-cyan-300/80" {...props} />,
    blockquote: (props: any) => (
      <blockquote className="border-l-4 border-cyan-500 pl-4 my-4 italic text-cyan-300/60" {...props} />
    ),
    code: ({ className, ...props }: any) => {
      const match = /language-(\w+)/.exec(className || '');
      
      if (!className) {
        return <code className="bg-black/50 px-1 py-0.5 rounded text-cyan-400 text-sm font-mono" {...props} />;
      }
      
      return (
        <pre className="bg-black/50 border border-cyan-500/20 rounded-lg p-4 overflow-x-auto mb-4">
          <code className={`hljs ${className} text-sm`} {...props} />
        </pre>
      );
    },
    img: (props: any) => (
      <img className="w-full rounded-lg my-4 border border-cyan-500/20" {...props} />
    ),
    hr: () => <hr className="border-cyan-500/20 my-8" />,
    ...components,
  };
}