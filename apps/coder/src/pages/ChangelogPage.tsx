import React from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui';

export default function ChangelogPage() {
  return (
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
            <h3 className="text-lg font-semibold mb-2">Version 1.0.0</h3>
            <ul className="list-disc pl-5 space-y-2">
              <li>Initial release</li>
              <li>Added support for multiple AI models</li>
              <li>Implemented real-time chat interface</li>
              <li>Added model configuration options</li>
            </ul>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
