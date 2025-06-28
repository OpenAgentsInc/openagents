import type { Meta, StoryObj } from '@storybook/react';
import { BlogPostList } from './BlogPostList';

const meta = {
  title: 'MVP/Organisms/Blog/BlogPostList',
  component: BlogPostList,
  parameters: {
    layout: 'fullscreen',
    backgrounds: {
      default: 'dark',
    },
  },
  tags: ['autodocs'],
  argTypes: {
    title: {
      control: 'text',
      description: 'Main title for the blog section',
    },
    description: {
      control: 'text',
      description: 'Description text below the title',
    },
    posts: {
      control: 'object',
      description: 'Array of blog posts to display',
    },
  },
} satisfies Meta<typeof BlogPostList>;

export default meta;
type Story = StoryObj<typeof meta>;

const samplePosts = [
  {
    slug: 'intro-to-ai-coding-agents',
    title: 'Intro to AI Coding Agents',
    date: '2025-04-30',
    summary: 'Our PlebLab workshop has equal parts education and shit talking.',
    image: '/images/blog01.png',
  },
  {
    slug: 'agent-payments-api',
    title: 'Introducing the Agent Payments API',
    date: '2025-05-05',
    summary: 'We built the easiest way for AI agents to send and receive money. Open beta is now available globally for all developers.',
    image: '/images/blog02.png',
  },
  {
    slug: 'ai-agents-at-off-2025',
    title: 'AI Agents @ OFF 2025',
    date: '2025-06-02',
    summary: 'We gave an introductory talk about AI agents at the Oslo Freedom Forum.',
    image: '/images/blog06.png',
  },
];

export const Default: Story = {
  args: {
    posts: samplePosts,
  },
};

export const CustomHeader: Story = {
  args: {
    title: 'Latest Updates',
    description: 'Stay up to date with the latest developments in AI agents and Bitcoin integration.',
    posts: samplePosts,
  },
};

export const SinglePost: Story = {
  args: {
    posts: [samplePosts[0]],
  },
};

export const EmptyState: Story = {
  args: {
    posts: [],
  },
};

export const ManyPosts: Story = {
  args: {
    posts: [
      ...samplePosts,
      {
        slug: 'building-with-effect',
        title: 'Building with Effect: A Functional Approach',
        date: '2025-05-15',
        summary: 'Learn how to build robust applications using Effect, a powerful TypeScript library for functional programming.',
      },
      {
        slug: 'bitcoin-lightning-integration',
        title: 'Bitcoin Lightning Integration Guide',
        date: '2025-05-20',
        summary: 'Step-by-step guide to integrating Bitcoin Lightning payments into your applications.',
      },
      {
        slug: 'open-source-ai-agents',
        title: 'The Future of Open Source AI Agents',
        date: '2025-05-25',
        summary: 'Why open source is crucial for the development of trustworthy AI agents.',
      },
    ],
  },
};