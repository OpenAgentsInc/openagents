import React from 'react';
import { Text, Animator, AnimatorGeneralProvider, Animated } from '@arwes/react';
import { BlogPostCard, BlogPostCardProps } from '../../molecules/blog/BlogPostCard';

export interface BlogPostListProps {
  posts: BlogPostCardProps[];
  title?: string;
  description?: string;
}

export const BlogPostList: React.FC<BlogPostListProps> = ({
  posts,
  title = 'Blog',
  description = 'Insights into building AI agents, Bitcoin integration, and the future of autonomous software.'
}) => {
  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <div className="max-w-6xl mx-auto">
        {/* Header */}
        <Animator active={true}>
          <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
            <div className="mb-12 text-center">
              <Text as="h1" className="text-4xl md:text-5xl font-bold text-cyan-100 mb-4">
                {title}
              </Text>
              <Text className="text-lg text-cyan-300/60 max-w-2xl mx-auto">
                {description}
              </Text>
            </div>
          </Animated>
        </Animator>
        
        {/* Blog posts grid */}
        <Animator active={true} duration={{ delay: 0.2 }}>
          <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
              {posts.map((post, index) => (
                <Animator key={post.slug} active={true} duration={{ delay: 0.1 * (index + 1) }}>
                  <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
                    <BlogPostCard {...post} />
                  </Animated>
                </Animator>
              ))}
            </div>
          </Animated>
        </Animator>
        
        {/* Empty state */}
        {posts.length === 0 && (
          <Animator active={true} duration={{ delay: 0.2 }}>
            <Animated animated={[['opacity', 0, 1]]}>
              <div className="text-center py-12">
                <Text className="text-cyan-300/60">
                  No blog posts yet. Check back soon!
                </Text>
              </div>
            </Animated>
          </Animator>
        )}
      </div>
    </AnimatorGeneralProvider>
  );
};