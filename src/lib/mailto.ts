import { looksLikeEmail, parseAddresses } from '@/lib/addresses';
import { api } from '@/lib/api';
import type { Address } from '@/lib/bindings';
import { textToHtml, useComposer } from '@/lib/composer';
import { isTauri } from '@/lib/tauri';
import { toastError } from '@/lib/toasts';

export interface MailtoMessage {
  to: Address[];
  cc: Address[];
  bcc: Address[];
  subject: string;
  body: string;
}

function decode(text: string): string {
  try {
    return decodeURIComponent(text);
  } catch {
    return text;
  }
}

function people(text: string): Address[] {
  const seen = new Set<string>();
  return parseAddresses(text).filter((a) => {
    if (!looksLikeEmail(a.email) || seen.has(a.email)) return false;
    seen.add(a.email);
    return true;
  });
}

function clean(text: string, keepNewlines: boolean): string {
  let out = '';
  for (const ch of text) {
    const code = ch.charCodeAt(0);
    const control = code < 32 || code === 127;
    if (!control || (keepNewlines && (ch === '\n' || ch === '\t'))) out += ch;
  }
  return out;
}

export function parseMailto(url: string): MailtoMessage | null {
  const trimmed = url.trim();
  if (!/^mailto:/i.test(trimmed)) return null;
  const rest = trimmed.slice('mailto:'.length);
  const at = rest.indexOf('?');
  const path = at === -1 ? rest : rest.slice(0, at);
  const query = at === -1 ? '' : rest.slice(at + 1);

  const message: MailtoMessage = {
    to: people(decode(path)),
    cc: [],
    bcc: [],
    subject: '',
    body: '',
  };
  for (const pair of query.split('&')) {
    const eq = pair.indexOf('=');
    if (eq === -1) continue;
    const key = pair.slice(0, eq).toLowerCase();
    const value = decode(pair.slice(eq + 1));
    if (key === 'to') message.to = people([...message.to.map((a) => a.email), value].join(','));
    else if (key === 'cc') message.cc = people(value);
    else if (key === 'bcc') message.bcc = people(value);
    else if (key === 'subject')
      message.subject = clean(value.replace(/[\r\n]+/g, ' '), false).trim();
    else if (key === 'body') message.body = clean(value.replace(/\r\n/g, '\n'), true);
  }
  return message;
}

export async function openMailto(url: string): Promise<void> {
  const message = parseMailto(url);
  if (!message) return;
  try {
    const draft = await api.newDraft(message.to);
    const extras =
      message.cc.length > 0 ||
      message.bcc.length > 0 ||
      message.subject !== '' ||
      message.body !== '';
    if (!extras) {
      useComposer.getState().show(draft.id, message.to.length > 0 ? 'body' : 'to', draft);
      return;
    }
    await api.saveDraft({
      id: draft.id,
      to: message.to,
      cc: message.cc,
      bcc: message.bcc,
      subject: message.subject,
      body_html: textToHtml(message.body),
      nudge_days: draft.nudge_days,
    });
    useComposer.getState().show(draft.id, message.to.length > 0 ? 'body' : 'to');
  } catch (error) {
    toastError(error);
  }
}

declare global {
  interface Window {
    __ZSM_MAILTO__?: string;
  }
}

export async function listenForMailto(): Promise<() => void> {
  if (!isTauri()) return () => {};
  const first = window.__ZSM_MAILTO__;
  if (first) {
    window.__ZSM_MAILTO__ = undefined;
    void openMailto(first);
  }
  const { listen } = await import('@tauri-apps/api/event');
  return listen<string>('zsm-mailto', (event) => void openMailto(event.payload));
}
