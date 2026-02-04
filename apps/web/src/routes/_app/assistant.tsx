import { createFileRoute, redirect } from '@tanstack/react-router';

export const Route = createFileRoute('/_app/assistant')({
  beforeLoad: () => {
    throw redirect({ to: '/chat/$chatId', params: { chatId: 'new' } });
  },
});
