import React from 'react';
import Link from 'next/link';
import { Text, FrameCorners, cx } from '@arwes/react';
import { CalendarDays } from 'lucide-react';

export interface BlogPostCardProps {
  slug: string;
  title: string;
  date: string;
  summary?: string;
}

export const BlogPostCard: React.FC<BlogPostCardProps> = ({
  slug,
  title,
  date,
  summary
}) => {
  return (
    <Link href={`/blog/${slug}`} className="block group">
      <div className="relative overflow-hidden rounded-lg transition-all duration-300 hover:scale-[1.02]">
        {/* Frame effect on hover */}
        <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none">
          <FrameCorners
            style={{
              '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.5)',
              '--arwes-frames-line-color': 'hsla(180, 100%, 75%, 0.5)',
              '--arwes-frames-deco-color': 'hsla(180, 100%, 75%, 1)'
            } as React.CSSProperties}
            cornerLength={20}
            strokeWidth={1}
          />
        </div>
        
        {/* Card content */}
        <div className="relative bg-black/30 border border-cyan-500/20 p-6 h-full">
          <Text as="h3" className="text-xl font-semibold text-cyan-100 mb-2 group-hover:text-cyan-300 transition-colors">
            {title}
          </Text>
          
          {summary && (
            <Text className="text-sm text-cyan-300/60 mb-4 line-clamp-2">
              {summary}
            </Text>
          )}
          
          <div className="flex items-center gap-2 text-xs text-cyan-500/60">
            <CalendarDays size={14} />
            <Text>{new Date(date).toLocaleDateString('en-US', { 
              month: 'long', 
              day: 'numeric', 
              year: 'numeric' 
            })}</Text>
          </div>
        </div>
      </div>
    </Link>
  );
};