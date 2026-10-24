export interface DevOrigins {
  editorPort: number;
  previewPort: number;
  editorOrigin: string;
  previewOrigin: string;
}
export function devOrigins(
  environment?: Record<string, string | undefined>,
): DevOrigins;
