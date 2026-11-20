import { QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { router } from '@/app/router';
import { api } from '@/lib/api';
import { installWebBackend } from '@/lib/ipc';
import { qk, queryClient } from '@/lib/queries';
import { showWindowAfterFirstPaint } from '@/lib/tauri';
import { warmFallbackFonts } from '@/lib/warmFonts';
import { MotionProvider } from '@/motion/MotionProvider';
import '@/styles/index.css';

const container = document.getElementById('root');
if (!container) throw new Error('index.html has no #root element');

installWebBackend();

try {
  await queryClient.prefetchQuery({ queryKey: qk.appState, queryFn: () => api.appState() });
} catch {}

createRoot(container).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <MotionProvider>
        <RouterProvider router={router} />
      </MotionProvider>
    </QueryClientProvider>
  </StrictMode>,
);

void showWindowAfterFirstPaint();

if (!new URLSearchParams(window.location.search).has('nowarm')) warmFallbackFonts();
