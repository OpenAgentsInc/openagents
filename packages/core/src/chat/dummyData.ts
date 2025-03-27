import { UIMessage } from './types';

export const dummyMessages: UIMessage[] = [
  {
    id: 'sys-1',
    role: 'system',
    content: 'I am an AI assistant that helps with coding tasks.',
    parts: [
      {
        type: 'text',
        text: 'I am an AI assistant that helps with coding tasks.'
      }
    ]
  },
  {
    id: 'usr-1',
    role: 'user',
    content: 'Can you help me create a React component?',
    parts: [
      {
        type: 'text',
        text: 'Can you help me create a React component?'
      }
    ]
  },
  {
    id: 'ast-1',
    role: 'assistant',
    content: "I'll help you create a React component. First, let me search for some examples.",
    parts: [
      {
        type: 'text',
        text: "I'll help you create a React component. First, let me search for some examples."
      },
      {
        type: 'tool-invocation',
        toolInvocation: {
          state: 'call',
          toolCallId: 'search-1',
          toolName: 'codebase_search',
          args: {
            query: 'React component examples',
            target_directories: ['src/components']
          }
        }
      }
    ]
  },
  {
    id: 'ast-2',
    role: 'assistant',
    content: 'Here is a simple example of a React component:',
    parts: [
      {
        type: 'reasoning',
        reasoning: 'Based on the search results, I will show a basic React component structure.',
        details: [
          {
            type: 'text',
            text: 'Analyzing search results to provide a clear example.'
          }
        ]
      },
      {
        type: 'text',
        text: 'Here is a simple example of a React component:'
      },
      {
        type: 'tool-invocation',
        toolInvocation: {
          state: 'result',
          toolCallId: 'edit-1',
          toolName: 'edit_file',
          args: {
            target_file: 'src/components/Example.tsx',
            code_edit: `
interface ExampleProps {
  title: string;
  description?: string;
}

export const Example = ({ title, description }: ExampleProps) => {
  return (
    <div className="example">
      <h2>{title}</h2>
      {description && <p>{description}</p>}
    </div>
  );
};`
          },
          result: 'File created successfully'
        }
      }
    ]
  },
  {
    id: 'usr-2',
    role: 'user',
    content: 'Can you add some styling to it?',
    parts: [
      {
        type: 'text',
        text: 'Can you add some styling to it?'
      }
    ]
  },
  {
    id: 'ast-3',
    role: 'assistant',
    content: "I'll add some CSS styling to make it look better.",
    parts: [
      {
        type: 'text',
        text: "I'll add some CSS styling to make it look better."
      },
      {
        type: 'tool-invocation',
        toolInvocation: {
          state: 'call',
          toolCallId: 'edit-2',
          toolName: 'edit_file',
          args: {
            target_file: 'src/components/Example.tsx',
            code_edit: `
const styles = {
  example: {
    padding: '20px',
    border: '1px solid #eaeaea',
    borderRadius: '8px',
    margin: '20px 0',
    boxShadow: '0 2px 4px rgba(0,0,0,0.1)'
  },
  title: {
    color: '#333',
    marginBottom: '10px'
  },
  description: {
    color: '#666',
    lineHeight: '1.5'
  }
};

interface ExampleProps {
  title: string;
  description?: string;
}

export const Example = ({ title, description }: ExampleProps) => {
  return (
    <div style={styles.example}>
      <h2 style={styles.title}>{title}</h2>
      {description && <p style={styles.description}>{description}</p>}
    </div>
  );
};`
          }
        }
      }
    ]
  }
];

// Example of a message with file attachment
export const messageWithAttachment: UIMessage = {
  id: 'usr-file-1',
  role: 'user',
  content: 'Here is the configuration file you requested:',
  experimental_attachments: [
    {
      name: 'config.json',
      contentType: 'application/json',
      url: 'data:application/json;base64,eyJuYW1lIjoiZXhhbXBsZSIsInZlcnNpb24iOiIxLjAuMCJ9'
    }
  ],
  parts: [
    {
      type: 'text',
      text: 'Here is the configuration file you requested:'
    },
    {
      type: 'file',
      mimeType: 'application/json',
      data: 'eyJuYW1lIjoiZXhhbXBsZSIsInZlcnNpb24iOiIxLjAuMCJ9'
    }
  ]
};

// Example of a message with source citation
export const messageWithSource: UIMessage = {
  id: 'ast-src-1',
  role: 'assistant',
  content: 'According to the documentation...',
  parts: [
    {
      type: 'text',
      text: 'According to the documentation...'
    },
    {
      type: 'source',
      source: {
        sourceType: 'url',
        id: 'doc-1',
        url: 'https://example.com/docs',
        title: 'API Documentation'
      }
    }
  ]
};
