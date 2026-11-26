import { useRef, useState } from 'react';
import { api } from '@/lib/api';
import type { ImportReport } from '@/lib/bindings';
import { plural } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { useAppState } from '@/lib/queries';
import { isTauri } from '@/lib/tauri';
import { toast, toastError } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { SectionHeading } from '@/ui/SectionHeading';

export function ContactsSection() {
  const t = useT();
  const input = useRef<HTMLInputElement>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [busy, setBusy] = useState(false);
  const hasGmail = (useAppState().data?.accounts ?? []).some((a) => a.provider === 'gmail');

  async function fromGoogle() {
    try {
      const url = await api.beginContactsImport();
      if (isTauri()) await api.openLink(url);
      else window.open(url, '_blank', 'noopener');
      toast('Finish in your browser. ZSortMail reads your contacts once and keeps no access.');
    } catch (e) {
      toastError(e);
    }
  }

  async function read(file: File) {
    setBusy(true);
    setReport(null);
    try {
      setReport(await api.importContacts(await file.text()));
    } catch (e) {
      toastError(e);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <SectionHeading>{t('settings.contacts')}</SectionHeading>
      <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 border-b border-rule py-4">
        <div className="max-w-[52ch]">
          <h3 className="text-body font-medium">Import an address book</h3>
          <p className="mt-0.5 text-ui text-text-2">
            Export your contacts as a vCard (.vcf) file from Google Contacts or any address book.
            ZSortMail fills in the role, organisation and notes of people you already write with. It
            never changes a name or role you set, and it never adds people.
          </p>
        </div>
        <input
          ref={input}
          type="file"
          accept=".vcf,text/vcard,text/x-vcard"
          aria-label="Choose a vCard file"
          hidden
          onChange={(e) => {
            const file = e.target.files?.[0];
            if (file) void read(file);
            e.target.value = '';
          }}
        />
        <Button disabled={busy} onClick={() => input.current?.click()}>
          Choose a file
        </Button>
      </div>
      {hasGmail && (
        <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 border-b border-rule py-4">
          <div className="max-w-[52ch]">
            <h3 className="text-body font-medium">Import from Google Contacts</h3>
            <p className="mt-0.5 text-ui text-text-2">
              Google asks you once for read-only access to your contacts. ZSortMail reads them,
              fills in roles and organisations for people you already write with, and keeps no
              access afterwards.
            </p>
          </div>
          <Button onClick={() => void fromGoogle()}>Import from Google</Button>
        </div>
      )}
      {report && (
        <p role="status" className="mt-3 text-ui text-text-2">
          Read {plural(report.contacts, 'contact')}. {plural(report.matched, 'person', 'people')}{' '}
          matched, {plural(report.updated, 'person', 'people')} updated,{' '}
          {plural(report.unmatched, 'contact')} had no matching person.
        </p>
      )}
    </>
  );
}
