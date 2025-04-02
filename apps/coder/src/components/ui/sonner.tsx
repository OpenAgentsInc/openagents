import { Toaster as Sonner, ToasterProps } from "sonner"

const Toaster = ({ ...props }: ToasterProps) => {
  return (
    <Sonner
      theme="light"
      className="toaster group !font-mono"
      style={{
        '--normal-bg': 'white',
        '--normal-text': 'black',
        '--normal-border': '#e2e8f0',
        '--success-bg': '#dcfce7',
        '--success-text': '#166534',
        '--error-bg': '#fee2e2',
        '--error-text': '#991b1b',
      } as React.CSSProperties}
      {...props}
    />
  )
}

export { Toaster }
