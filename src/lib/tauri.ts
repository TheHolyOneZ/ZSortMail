declare global {
  interface Window {
    __ZSM_WEB__?: true;
  }
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window && !window.__ZSM_WEB__;
}

export function zsmUrl(path: string): string {
  if (!isTauri()) return `/zsm/${path}`;
  const windows = typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent);
  return windows ? `http://zsm.localhost/${path}` : `zsm://localhost/${path}`;
}

export async function showWindowAfterFirstPaint(): Promise<void> {
  if (!isTauri()) return;
  await new Promise<void>((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
  );
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().show();
}
