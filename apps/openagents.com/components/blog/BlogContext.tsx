'use client'

import React, { createContext, useContext } from 'react';

interface BlogMetadata {
  title?: string;
  date?: string;
  summary?: string;
  readingTime?: string;
}

const BlogContext = createContext<BlogMetadata>({});

export const useBlogMetadata = () => useContext(BlogContext);

export const BlogProvider: React.FC<{ 
  metadata: BlogMetadata; 
  children: React.ReactNode; 
}> = ({ metadata, children }) => {
  return (
    <BlogContext.Provider value={metadata}>
      {children}
    </BlogContext.Provider>
  );
};