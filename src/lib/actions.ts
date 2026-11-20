import { api } from '@/lib/api';
import type { Done } from '@/lib/bindings';
import { toast, toastError } from '@/lib/toasts';

export async function undo(): Promise<void> {
  try {
    const what = await api.undo();
    toast(what ? `Undone: ${what}.` : 'Nothing to undo.');
  } catch (error) {
    toastError(error);
  }
}

export async function act(run: () => Promise<Done>): Promise<boolean> {
  try {
    const done = await run();
    toast(`${done.label}.`, done.can_undo ? { label: 'Undo', run: () => void undo() } : undefined);
    return true;
  } catch (error) {
    toastError(error);
    return false;
  }
}
