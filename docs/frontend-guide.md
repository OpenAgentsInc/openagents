# Frontend Development Guide

This guide covers the frontend architecture and development practices in OpenAgents.

## Component Organization

```
resources/js/
├── Components/          # Shared components
│   ├── ui/             # Basic UI components
│   └── forms/          # Form-related components
├── Layouts/            # Page layouts
├── Pages/             # Page components
├── hooks/             # Custom React hooks
├── lib/              # Utility functions
└── types/            # TypeScript definitions
```

## Key Technologies

1. **React with TypeScript**
- Strict type checking enabled
- Interface definitions for all props
- Shared type definitions in `types/`

2. **Radix UI Components**
We use Radix UI for accessible, headless components:
```typescript
import * as Dialog from '@radix-ui/react-dialog';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
// etc.
```

3. **Styling**
- Tailwind CSS for utility-first styling
- class-variance-authority for component variants
- tailwind-merge for class conflicts

## Component Patterns

1. **Basic Component Structure**
```typescript
interface ButtonProps {
  variant?: 'primary' | 'secondary';
  size?: 'sm' | 'md' | 'lg';
  children: React.ReactNode;
}

export const Button = ({
  variant = 'primary',
  size = 'md',
  children
}: ButtonProps) => {
  // Implementation
};
```

2. **Using Variants**
```typescript
import { cva } from 'class-variance-authority';

const buttonVariants = cva(
  'rounded-md font-medium transition-colors',
  {
    variants: {
      variant: {
        primary: 'bg-blue-500 text-white hover:bg-blue-600',
        secondary: 'bg-gray-100 text-gray-900 hover:bg-gray-200'
      },
      size: {
        sm: 'px-3 py-1.5 text-sm',
        md: 'px-4 py-2 text-base',
        lg: 'px-6 py-3 text-lg'
      }
    },
    defaultVariants: {
      variant: 'primary',
      size: 'md'
    }
  }
);
```

## Forms and Validation

1. **Form Structure**
```typescript
import { useForm } from '@inertiajs/react';

interface FormData {
  name: string;
  email: string;
}

export const UserForm = () => {
  const { data, setData, post, processing, errors } = useForm<FormData>({
    name: '',
    email: ''
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    post('/users');
  };
};
```

2. **File Uploads**
```typescript
import { useDropzone } from 'react-dropzone';

export const FileUpload = () => {
  const { getRootProps, getInputProps } = useDropzone({
    accept: {
      'application/pdf': ['.pdf']
    },
    maxSize: 5000000 // 5MB
  });
};
```

## State Management

1. **Local State**
```typescript
const [isOpen, setIsOpen] = useState(false);
```

2. **Server State**
```typescript
import { usePage } from '@inertiajs/react';

interface PageProps {
  user: User;
  projects: Project[];
}

const { user, projects } = usePage<PageProps>().props;
```

## Custom Hooks

1. **API Hooks**
```typescript
export const useProjects = () => {
  const { data, setData, post, processing } = useForm({
    name: '',
    description: ''
  });

  const createProject = () => post('/projects');

  return { data, setData, createProject, processing };
};
```

2. **Utility Hooks**
```typescript
export const useDebounce = <T>(value: T, delay: number): T => {
  const [debouncedValue, setDebouncedValue] = useState<T>(value);

  useEffect(() => {
    const handler = setTimeout(() => {
      setDebouncedValue(value);
    }, delay);

    return () => {
      clearTimeout(handler);
    };
  }, [value, delay]);

  return debouncedValue;
};
```

## Testing

```typescript
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

describe('Button', () => {
  it('renders correctly', () => {
    render(<Button>Click me</Button>);
    expect(screen.getByText('Click me')).toBeInTheDocument();
  });
});
```

## Best Practices

1. **Component Organization**
- One component per file
- Group related components in folders
- Use index.ts files for exports

2. **Type Safety**
- Define interfaces for all props
- Use strict TypeScript settings
- Avoid any types

3. **Performance**
- Use React.memo for expensive renders
- Implement proper dependency arrays in useEffect
- Code-split large components

4. **Accessibility**
- Use semantic HTML
- Implement ARIA attributes
- Test with keyboard navigation