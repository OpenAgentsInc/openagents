import type { MDXComponents } from 'mdx/types';
import dynamic from 'next/dynamic';

const MDXTwitterEmbed = dynamic(() => import('./components/blog/MDXTwitterEmbed').then(mod => mod.MDXTwitterEmbed), {
  ssr: false,
  loading: () => <div className="w-full h-[400px] bg-black/50 animate-pulse rounded-lg my-8" />
});

// @ts-ignore
export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    // @ts-ignore
    h1: (props: any) => <h1 className="text-3xl font-bold text-cyan-100 mt-8 mb-4" {...props} />,
    // @ts-ignore
    h2: (props: any) => <h2 className="text-2xl font-semibold text-cyan-100 mt-6 mb-3" {...props} />,
    // @ts-ignore
    h3: (props: any) => <h3 className="text-xl font-semibold text-cyan-200 mt-4 mb-2" {...props} />,
    // @ts-ignore
    p: (props: any) => <p className="text-cyan-300/80 mb-4 leading-relaxed" {...props} />,
    // @ts-ignore
    a: (props: any) => <a className="text-cyan-400 hover:text-cyan-300 underline transition-colors" {...props} />,
    // @ts-ignore
    ul: (props: any) => <ul className="list-disc list-inside text-cyan-300/80 mb-4 space-y-1" {...props} />,
    // @ts-ignore
    ol: (props: any) => <ol className="list-decimal list-inside text-cyan-300/80 mb-4 space-y-1" {...props} />,
    // @ts-ignore
    li: (props: any) => <li className="text-cyan-300/80" {...props} />,
    // @ts-ignore
    blockquote: (props: any) => (
      <blockquote className="border-l-4 border-cyan-500 pl-4 my-4 italic text-cyan-300/60" {...props} />
    ),
    // @ts-ignore
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
    // @ts-ignore
    img: (props: any) => (
      <img className="w-full rounded-lg my-4 border border-cyan-500/20" {...props} />
    ),
    // @ts-ignore
    hr: () => <hr className="border-cyan-500/20 my-8" />,
    // @ts-ignore
    TwitterEmbed: MDXTwitterEmbed,
    ...components,
  };
}