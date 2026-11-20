import { useEffect, useState } from 'react';
import { api } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { qk, queryClient, useComposeSettings } from '@/lib/queries';
import { toastError } from '@/lib/toasts';
import { Select } from '@/ui/Select';

const UNDO_CHOICES = [0, 5, 10, 20, 30];

export function WritingSection() {
  const t = useT();
  const { data: settings } = useComposeSettings();
  const [signature, setSignature] = useState('');

  useEffect(() => {
    if (settings) setSignature(settings.signature);
  }, [settings]);

  if (!settings) return null;

  async function save(next: { signature: string; undo_seconds: number }) {
    queryClient.setQueryData(qk.composeSettings, next);
    try {
      await api.saveComposeSettings(next);
    } catch (e) {
      toastError(e);
    } finally {
      await queryClient.invalidateQueries({ queryKey: qk.composeSettings });
    }
  }

  return (
    <>
      <h2 className="mt-8 text-name font-semibold">{t('settings.writing')}</h2>
      <div className="border-b border-rule py-4">
        <label htmlFor="signature" className="block text-body font-medium">
          Signature
        </label>
        <p className="mt-0.5 max-w-[52ch] text-ui text-text-2">
          Added to the end of every new message, reply and forward.
        </p>
        <textarea
          id="signature"
          value={signature}
          rows={3}
          dir="auto"
          onChange={(e) => setSignature(e.target.value)}
          onBlur={() => {
            if (signature !== settings.signature)
              void save({ signature, undo_seconds: settings.undo_seconds });
          }}
          className="mt-3 w-full max-w-[52ch] rounded-control bg-hover px-3 py-2 text-ui outline-none focus-visible:outline-2 focus-visible:outline-blue"
        />
      </div>
      <div className="flex items-start justify-between gap-6 border-b border-rule py-4">
        <div className="max-w-[52ch]">
          <span className="block text-body font-medium">Undo send</span>
          <p className="mt-0.5 text-ui text-text-2">
            After you press Send, the message waits this long before it leaves, so you can take it
            back.
          </p>
        </div>
        <Select<number>
          label="Undo send"
          className="w-44"
          align="end"
          value={settings.undo_seconds}
          onChange={(v) => void save({ signature: settings.signature, undo_seconds: v })}
          options={UNDO_CHOICES.map((s) => ({
            value: s,
            label: s === 0 ? 'Send at once' : `${s} seconds`,
          }))}
        />
      </div>
    </>
  );
}
