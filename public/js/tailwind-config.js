tailwind.config = {
    darkMode: 'class',
    theme: {
        extend: {
            fontFamily: {
                mono: ['"JetBrains Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'Monaco', 'Consolas', '"Liberation Mono"', '"Courier New"', 'monospace'],
            },
            colors: {
                zinc: {
                    50: 'hsl(0, 0%, 98%)',
                    100: 'hsl(240, 4.8%, 95.9%)',
                    200: 'hsl(240, 5.9%, 90%)',
                    300: 'hsl(240, 4.9%, 83.9%)',
                    400: 'hsl(240, 5%, 64.9%)',
                    500: 'hsl(240, 3.8%, 46.1%)',
                    600: 'hsl(240, 5.2%, 33.9%)',
                    700: 'hsl(240, 5.3%, 26.1%)',
                    800: 'hsl(240, 3.7%, 15.9%)',
                    900: 'hsl(240, 5.9%, 10%)',
                    950: 'hsl(240, 10%, 3.9%)',
                },
                background: 'hsl(var(--background))',
                foreground: 'hsl(var(--foreground))',
                primary: {
                    DEFAULT: 'hsl(var(--primary))',
                    foreground: 'hsl(var(--primary-foreground))',
                },
                secondary: {
                    DEFAULT: 'hsl(var(--secondary))',
                    foreground: 'hsl(var(--secondary-foreground))',
                },
                destructive: {
                    DEFAULT: 'hsl(var(--destructive))',
                    foreground: 'hsl(var(--destructive-foreground))',
                },
                muted: {
                    DEFAULT: 'hsl(var(--muted))',
                    foreground: 'hsl(var(--muted-foreground))',
                },
                accent: {
                    DEFAULT: 'hsl(var(--accent))',
                    foreground: 'hsl(var(--accent-foreground))',
                },
                popover: {
                    DEFAULT: 'hsl(var(--popover))',
                    foreground: 'hsl(var(--popover-foreground))',
                },
                card: {
                    DEFAULT: 'hsl(var(--card))',
                    foreground: 'hsl(var(--card-foreground))',
                },
                border: 'hsl(var(--border))',
                input: 'hsl(var(--input))',
                ring: 'hsl(var(--ring))',
                sidebar: {
                    background: 'var(--sidebar-background)',
                    foreground: 'var(--sidebar-foreground)',
                    primary: 'var(--sidebar-primary)',
                    'primary-foreground': 'var(--sidebar-primary-foreground)',
                    accent: 'var(--sidebar-accent)',
                    'accent-foreground': 'var(--sidebar-accent-foreground)',
                    border: 'var(--sidebar-border)',
                    ring: 'var(--sidebar-ring)',
                },
            },
            borderRadius: {
                lg: 'var(--radius)',
                md: 'calc(var(--radius) - 2px)',
                sm: 'calc(var(--radius) - 4px)',
            },
        }
    }
}