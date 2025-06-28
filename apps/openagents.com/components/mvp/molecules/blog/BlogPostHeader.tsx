import React from 'react';
import { Text } from '@arwes/react';
import { CalendarDays, Clock } from 'lucide-react';

export interface BlogPostHeaderProps {
  title: string;
  date: string;
  summary?: string;
  image?: string;
  readingTime: string;
}

export const BlogPostHeader: React.FC<BlogPostHeaderProps> = ({
  title,
  date,
  summary,
  image,
  readingTime
}) => {
  return (
    <header className="mb-8">
      {/* Featured image */}
      {image && (
        <div className="mb-8 -mx-8 -mt-12">
          <img 
            src={image} 
            alt={title}
            className="w-full h-64 md:h-96 object-cover"
          />
        </div>
      )}
      
      <Text as="h1" className="text-3xl md:text-4xl font-bold text-cyan-100 mb-4">
        {title}
      </Text>
      
      {summary && (
        <Text className="text-lg text-cyan-300/60 mb-4">
          {summary}
        </Text>
      )}
      
      <div className="flex items-center gap-4 text-sm text-cyan-500/60">
        <div className="flex items-center gap-2">
          <CalendarDays size={16} />
          <Text>{new Date(date).toLocaleDateString('en-US', { 
            month: 'long', 
            day: 'numeric', 
            year: 'numeric' 
          })}</Text>
        </div>
        <div className="flex items-center gap-2">
          <Clock size={16} />
          <Text>{readingTime}</Text>
        </div>
      </div>
    </header>
  );
};