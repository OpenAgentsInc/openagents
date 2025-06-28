import type { Meta, StoryObj } from '@storybook/react';
import { BlogPostHeader } from './BlogPostHeader';

const meta = {
  title: 'MVP/Molecules/Blog/BlogPostHeader',
  component: BlogPostHeader,
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
      description: 'Blog post title',
    },
    date: {
      control: 'date',
      description: 'Publication date',
    },
    summary: {
      control: 'text',
      description: 'Short description of the blog post',
    },
    image: {
      control: 'text',
      description: 'Featured image URL',
    },
    readingTime: {
      control: 'text',
      description: 'Estimated reading time',
    },
  },
} satisfies Meta<typeof BlogPostHeader>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {
    title: 'Introducing the Agent Payments API',
    date: '2025-05-05',
    summary: 'We built the easiest way for AI agents to send and receive money. Open beta is now available globally for all developers.',
    image: '/images/blog02.png',
    readingTime: '5 min read',
  },
};

export const WithoutImage: Story = {
  args: {
    title: 'Building with Effect: A Functional Approach',
    date: '2025-05-15',
    summary: 'Learn how to build robust applications using Effect, a powerful TypeScript library for functional programming.',
    readingTime: '8 min read',
  },
};

export const LongTitle: Story = {
  args: {
    title: 'A Comprehensive Guide to Building AI Agents with Bitcoin Lightning Network Integration and Autonomous Payment Processing',
    date: '2025-06-01',
    summary: 'Everything you need to know about creating autonomous agents that can send and receive Bitcoin payments through the Lightning Network.',
    image: '/images/blog06.png',
    readingTime: '15 min read',
  },
};

export const MinimalContent: Story = {
  args: {
    title: 'Quick Update',
    date: '2025-06-10',
    readingTime: '2 min read',
  },
};