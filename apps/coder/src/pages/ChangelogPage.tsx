import React from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui';

export default function ChangelogPage() {
  return (
    <div className="h-full w-full overflow-auto p-6">
      <div className="max-w-3xl mx-auto">
        <Card className="font-mono">
          <CardHeader>
            <CardTitle>Changelog</CardTitle>
            <CardDescription>
              Version history and latest changes
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-6">
              <div>
                <h3 className="text-lg font-semibold mb-2">v0.0.1-rc1</h3>
                <p className="text-sm text-muted-foreground mb-4">First release candidate for Coder MVP</p>
                <ul className="list-disc pl-5 space-y-2">
                  <li>Basic AI chat interface with thread history</li>
                  <li>Support for multiple AI models via bring-your-own API key</li>
                  <li>Model Context Protocol (MCP) integration:
                    <ul className="list-circle pl-5 mt-1 space-y-1 text-sm">
                      <li>Support for both local and remote MCP servers</li>
                      <li>Configurable MCP clients through settings UI</li>
                      <li>GitHub tools integration via MCP</li>
                    </ul>
                  </li>
                  <li>Settings improvements:
                    <ul className="list-circle pl-5 mt-1 space-y-1 text-sm">
                      <li>Redesigned settings page with improved navigation</li>
                      <li>MCP client configuration management</li>
                      <li>Model settings and API key management</li>
                    </ul>
                  </li>
                  <li>UI enhancements:
                    <ul className="list-circle pl-5 mt-1 space-y-1 text-sm">
                      <li>Consistent layout between chat and settings</li>
                      <li>Improved navigation with back-to-chat functionality</li>
                      <li>Real-time status indicators for MCP connections</li>
                    </ul>
                  </li>
                </ul>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}