import { create } from 'zustand';
import { looksLikeEmail } from '@/lib/addresses';
import { api } from '@/lib/api';
import type { Address, Draft, DraftMode } from '@/lib/bindings';
import { toastError } from '@/lib/toasts';

interface ComposerState {
  draftId: number | null;
  minimized: boolean;
  focus: 'to' | 'body';
  initial: Draft | null;
  show: (draftId: number, focus?: 'to' | 'body', initial?: Draft) => void;
  close: () => void;
  setMinimized: (minimized: boolean) => void;
}

export const useComposer = create<ComposerState>()((set) => ({
  draftId: null,
  minimized: false,
  focus: 'to',
  initial: null,
  show: (draftId, focus = 'body', initial) =>
    set({ draftId, minimized: false, focus, initial: initial ?? null }),
  close: () => set({ draftId: null, minimized: false, initial: null }),
  setMinimized: (minimized) => set({ minimized }),
}));

export async function compose(to: Address[] = []): Promise<void> {
  try {
    const draft = await api.newDraft(to);
    useComposer.getState().show(draft.id, to.length > 0 ? 'body' : 'to', draft);
  } catch (error) {
    toastError(error);
  }
}

export async function categoryAddresses(id: number): Promise<Address[]> {
  const members = await api.listCategoryMembers(id);
  const seen = new Set<string>();
  const out: Address[] = [];
  for (const m of members) {
    const email = m.email.trim().toLowerCase();
    if (!looksLikeEmail(email) || seen.has(email)) continue;
    seen.add(email);
    out.push({ email, name: m.alias ?? m.person.name });
  }
  return out;
}

export async function writeToCategory(id: number, name: string): Promise<void> {
  try {
    const everyone = await categoryAddresses(id);
    if (everyone.length === 0) {
      toastError(new Error(`Nobody in ${name} has an email address yet.`));
      return;
    }
    const draft = await api.newDraft([]);
    await api.saveDraft({
      id: draft.id,
      to: [],
      cc: [],
      bcc: everyone,
      subject: draft.subject,
      body_html: draft.body_html,
      nudge_days: draft.nudge_days,
    });
    useComposer.getState().show(draft.id, 'body');
  } catch (error) {
    toastError(error);
  }
}

export async function writeToPerson(personId: number, name: string): Promise<void> {
  try {
    const person = await api.getPerson(personId);
    const email = person?.addresses[0];
    if (!email) {
      toastError(new Error(`${name} has no email address yet.`));
      return;
    }
    await compose([{ email, name }]);
  } catch (error) {
    toastError(error);
  }
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

export function textToHtml(text: string): string {
  return text
    .replace(/\r\n/g, '\n')
    .split(/\n{2,}/)
    .filter((p) => p.trim())
    .map((p) => `<p>${escapeHtml(p.trimEnd()).replace(/\n/g, '<br>')}</p>`)
    .join('');
}

export async function composeFor(
  conversationId: number,
  mode: DraftMode,
  typed = '',
): Promise<void> {
  try {
    const draft = await api.replyDraft(conversationId, mode);
    if (typed.trim()) {
      await api.saveDraft({
        id: draft.id,
        to: draft.to,
        cc: draft.cc,
        bcc: draft.bcc,
        subject: draft.subject,
        body_html: textToHtml(typed) + draft.body_html,
        nudge_days: draft.nudge_days,
      });
      useComposer.getState().show(draft.id, 'body');
      return;
    }
    useComposer.getState().show(draft.id, mode === 'forward' ? 'to' : 'body', draft);
  } catch (error) {
    toastError(error);
  }
}

export function openDraft(id: number): void {
  useComposer.getState().show(id, 'body');
}
