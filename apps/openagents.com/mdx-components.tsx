import type { MDXComponents } from 'mdx/types';

// Simple placeholder for Twitter embeds - will be enhanced with client-side loading
function TwitterEmbed({ html }: { html?: string }) {
  return (
    <div className="w-full h-[400px] bg-black/50 border border-cyan-500/20 rounded-lg my-8 flex items-center justify-center">
      <p className="text-cyan-300/60">Twitter embed will load here</p>
    </div>
  );
}

// @ts-ignore
export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    // Wrapper div for all MDX content
    wrapper: ({ children }: any) => (
      <div className="relative z-10 h-full overflow-y-auto">
        <div className="absolute inset-0 pointer-events-none">
          <div className="absolute inset-0" style={{
            backgroundImage: `
              linear-gradient(90deg, hsla(180, 100%, 75%, 0.02) 1px, transparent 1px),
              linear-gradient(180deg, hsla(180, 100%, 75%, 0.02) 1px, transparent 1px)
            `,
            backgroundSize: '40px 40px'
          }} />
          <div className="absolute inset-0" style={{
            backgroundImage: `radial-gradient(circle, hsla(180, 50%, 50%, 0.02) 1px, transparent 1px)`,
            backgroundSize: '30px 30px'
          }} />
        </div>
        
        <article className="relative max-w-4xl mx-auto px-8 py-12">
          <div className="prose prose-lg prose-invert prose-cyan max-w-none">
            {children}
          </div>
        </article>
      </div>
    ),
    // @ts-ignore
    h1: (props: any) => <h1 className="text-4xl font-bold text-cyan-100 mt-12 mb-6 first:mt-0" {...props} />,
    // @ts-ignore
    h2: (props: any) => <h2 className="text-3xl font-semibold text-cyan-100 mt-10 mb-5 border-b border-cyan-500/20 pb-2" {...props} />,
    // @ts-ignore
    h3: (props: any) => <h3 className="text-2xl font-semibold text-cyan-200 mt-8 mb-4" {...props} />,
    // @ts-ignore
    p: (props: any) => <p className="text-cyan-300/80 mb-6 leading-relaxed text-lg" {...props} />,
    // @ts-ignore
    a: (props: any) => <a className="text-cyan-400 hover:text-cyan-300 underline decoration-2 underline-offset-2 transition-colors" {...props} />,
    // @ts-ignore
    ul: (props: any) => <ul className="list-disc list-outside text-cyan-300/80 mb-6 space-y-2 ml-6" {...props} />,
    // @ts-ignore
    ol: (props: any) => <ol className="list-decimal list-outside text-cyan-300/80 mb-6 space-y-2 ml-6" {...props} />,
    // @ts-ignore
    li: (props: any) => <li className="text-cyan-300/80 leading-relaxed" {...props} />,
    // @ts-ignore
    blockquote: (props: any) => (
      <blockquote className="border-l-4 border-cyan-500 pl-6 my-8 italic text-cyan-300/70 bg-cyan-950/20 py-4 rounded-r-lg" {...props} />
    ),
    // @ts-ignore
    pre: ({ children, ...props }: any) => (
      <pre className="bg-black/70 border border-cyan-500/30 rounded-lg p-6 overflow-x-auto my-6 text-sm leading-relaxed" {...props}>
        {children}
      </pre>
    ),
    // @ts-ignore
    code: ({ className, ...props }: any) => {
      if (!className) {
        return <code className="bg-black/60 px-2 py-1 rounded text-cyan-400 text-sm font-mono border border-cyan-500/20" {...props} />;
      }
      
      return <code className={`text-cyan-100 font-mono ${className || ''}`} {...props} />;
    },
    // @ts-ignore
    img: (props: any) => (
      <img className="w-full max-w-3xl mx-auto rounded-lg my-8 border border-cyan-500/20 shadow-lg shadow-cyan-500/10" {...props} />
    ),
    // @ts-ignore
    table: (props: any) => (
      <div className="overflow-x-auto my-6">
        <table className="w-full border-collapse border border-cyan-500/20 rounded-lg" {...props} />
      </div>
    ),
    // @ts-ignore
    th: (props: any) => (
      <th className="border border-cyan-500/20 bg-cyan-950/30 px-4 py-2 text-left text-cyan-100 font-semibold" {...props} />
    ),
    // @ts-ignore
    td: (props: any) => (
      <td className="border border-cyan-500/20 px-4 py-2 text-cyan-300/80" {...props} />
    ),
    // @ts-ignore
    hr: () => <hr className="border-cyan-500/20 my-8" />,
    // @ts-ignore
    TwitterEmbed: TwitterEmbed,
    ...components,
  };
}