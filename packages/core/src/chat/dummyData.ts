import { UIMessage } from './types';

export const messages: UIMessage[] = [
  {
    id: 'sys-1',
    role: 'system',
    content: 'You are a helpful AI assistant.',
    createdAt: new Date(),
    parts: [
      {
        type: 'text',
        text: 'You are a helpful AI assistant.'
      }
    ]
  },
  {
    id: 'usr-1',
    role: 'user',
    content: 'Hello! Can you help me search through my codebase?',
    createdAt: new Date(),
    parts: [
      {
        type: 'text',
        text: 'Hello! Can you help me search through my codebase?'
      }
    ]
  },
  {
    id: 'ast-1',
    role: 'assistant',
    content: "I'll help you create a React component. First, let me search for some examples.",
    createdAt: new Date(),
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
    createdAt: new Date(),
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
    createdAt: new Date(),
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
    createdAt: new Date(),
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
  id: 'usr-3',
  role: 'user',
  content: 'Here is a file I want to share.',
  createdAt: new Date(),
  experimental_attachments: [
    {
      name: 'example.txt',
      contentType: 'text/plain',
      url: 'https://example.com/file.txt'
    }
  ],
  parts: [
    {
      type: 'text',
      text: 'Here is a file I want to share.'
    },
    {
      type: 'file',
      mimeType: 'text/plain',
      data: 'Example file content'
    }
  ]
};

// Example of a message with source citation
export const messageWithSource: UIMessage = {
  id: 'ast-4',
  role: 'assistant',
  content: 'I found this information from a reliable source.',
  createdAt: new Date(),
  parts: [
    {
      type: 'text',
      text: 'I found this information from a reliable source.'
    },
    {
      type: 'source',
      source: {
        sourceType: 'url',
        id: 'src-1',
        url: 'https://example.com',
        title: 'Example Source'
      }
    }
  ]
};
