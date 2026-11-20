import { useEffect, useRef, useState } from 'react';
import { api } from '@/lib/api';
import type { MessageView } from '@/lib/bindings';
import { zsmUrl } from '@/lib/tauri';

export function MailFrame({
  message,
  allowRemote,
  onLink,
}: {
  message: MessageView;
  allowRemote: boolean;
  onLink: (href: string, text: string) => void;
}) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [height, setHeight] = useState(96);
  const path = message.html_url;

  const inlineIds = message.attachments.filter((a) => a.inline).map((a) => a.id);
  const [ready, setReady] = useState(inlineIds.length === 0);
  const inlineKey = inlineIds.join(',');
  // biome-ignore lint/correctness/useExhaustiveDependencies: inlineKey stands for inlineIds.
  useEffect(() => {
    if (inlineIds.length === 0) {
      setReady(true);
      return;
    }
    let live = true;
    Promise.all(inlineIds.map((id) => api.prepareAttachment(id)))
      .catch(() => {})
      .finally(() => live && setReady(true));
    return () => {
      live = false;
    };
  }, [inlineKey]);

  useEffect(() => {
    function onMessage(event: MessageEvent) {
      if (event.source !== frame.current?.contentWindow) return;
      const data = event.data as { zsm?: string; height?: number; href?: string; text?: string };
      if (data?.zsm === 'frame' && typeof data.height === 'number') {
        setHeight(Math.min(Math.max(data.height, 24), 20_000));
      } else if (data?.zsm === 'link' && typeof data.href === 'string') {
        onLink(data.href, typeof data.text === 'string' ? data.text : '');
      }
    }
    window.addEventListener('message', onMessage);
    return () => window.removeEventListener('message', onMessage);
  }, [onLink]);

  if (!path) return null;
  if (!ready) return <div className="h-24" aria-busy="true" />;
  return (
    <iframe
      ref={frame}
      title={`Message from ${message.from.name}`}
      src={zsmUrl(allowRemote ? `${path}?remote=1` : path)}
      sandbox="allow-scripts"
      referrerPolicy="no-referrer"
      className="block w-full border-0 bg-paper"
      style={{ height, colorScheme: 'light' }}
    />
  );
}
