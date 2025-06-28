'use client'

import React from 'react';
import Link from 'next/link';
import { CalendarDays, Clock, ArrowLeft } from 'lucide-react';
import { useBlogMetadata } from './BlogContext';

// Blog post header component
export function BlogHeader() {
  const { title, date, summary, readingTime } = useBlogMetadata();
  
  return (
    <header className="mb-12 pb-8 border-b border-cyan-500/20">
      {/* Back to Blog button */}
      <Link 
        href="/blog" 
        className="inline-flex items-center gap-2 text-cyan-400 hover:text-cyan-300 transition-colors mb-8 group"
      >
        <ArrowLeft size={16} className="group-hover:-translate-x-1 transition-transform" />
        <span className="text-sm font-medium">Back to Blog</span>
      </Link>
      
      {/* Only show metadata if we have some */}
      {(title || date || summary) && (
        <>
          {title && (
            <h1 className="text-5xl font-bold text-cyan-100 mb-6 leading-tight">
              {title}
            </h1>
          )}
          
          {summary && (
            <p className="text-xl text-cyan-300/70 mb-6 leading-relaxed font-light">
              {summary}
            </p>
          )}
          
          <div className="flex items-center gap-6 text-sm text-cyan-500/60">
            {date && (
              <div className="flex items-center gap-2">
                <CalendarDays size={16} />
                <span>
                  {new Date(date).toLocaleDateString('en-US', { 
                    month: 'long', 
                    day: 'numeric', 
                    year: 'numeric' 
                  })}
                </span>
              </div>
            )}
            
            {readingTime && (
              <div className="flex items-center gap-2">
                <Clock size={16} />
                <span>{readingTime}</span>
              </div>
            )}
          </div>
        </>
      )}
    </header>
  );
}