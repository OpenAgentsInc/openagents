'use client';

import React from 'react';
import { GridLines, Dots } from '@arwes/react';
import { AppLayout } from '@/components/AppLayout';
import { OnboardingOverlayManager } from '@/components/onboarding/OnboardingOverlayManager';
import { BlogPostList } from '@/components/mvp/organisms/blog/BlogPostList';
import { getAllPosts } from './utils';

const BlogPage = (): React.ReactElement => {
  const posts = getAllPosts();
  
  return (
    <AppLayout showSidebar>
      <OnboardingOverlayManager
        minDesktopWidth={1024}
        desktopMessage="OpenAgents requires a desktop browser for the full development experience. Please use a device with a screen width of at least 1024px."
      >
        <div className="relative z-10 h-full overflow-y-auto">
          {/* Background effects */}
          <div className="absolute inset-0 pointer-events-none">
            <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
            <Dots color="hsla(180, 50%, 50%, 0.02)" size={1} distance={30} />
          </div>
          
          {/* Content container */}
          <div className="relative px-8 py-12">
            <BlogPostList posts={posts} />
          </div>
        </div>
      </OnboardingOverlayManager>
    </AppLayout>
  );
};

export default BlogPage;