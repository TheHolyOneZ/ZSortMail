import {
  DEFAULT_ANIMATIONS,
  DIALOG_OPTIONS,
  PAGE_OPTIONS,
  STAMP_OPTIONS,
  TOAST_OPTIONS,
} from '@/lib/animations';
import { useT } from '@/lib/i18n';
import { useUi } from '@/lib/store';
import { toast } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { Select } from '@/ui/Select';
import { Toggle } from '@/ui/Toggle';

function Row({
  title,
  hint,
  children,
}: {
  title: string;
  hint: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 border-b border-rule py-4">
      <div className="max-w-[48ch]">
        <h3 className="text-body font-medium">{title}</h3>
        <p className="mt-0.5 text-ui text-text-2">{hint}</p>
      </div>
      {children}
    </div>
  );
}

export function AnimationsSection() {
  const t = useT();
  const anim = useUi((s) => s.anim);
  const setAnim = useUi((s) => s.setAnim);

  return (
    <>
      <h2 className="mt-8 text-name font-semibold">{t('settings.animations')}</h2>
      <p className="mt-1 max-w-[56ch] text-ui text-text-2">
        Pick how things move, or turn a kind of movement off. The Motion setting above can still
        reduce all of them.
      </p>
      <Row
        title="Moving between pages"
        hint="When a narrow window shows one page at a time: People, then a person, then a conversation."
      >
        <Select
          label="Moving between pages"
          className="w-44"
          align="end"
          value={anim.page}
          onChange={(page) => setAnim({ page })}
          options={PAGE_OPTIONS}
        />
      </Row>
      <Row title="Dialogs" hint="How windows like Edit person and New rule appear.">
        <Select
          label="Dialogs"
          className="w-44"
          align="end"
          value={anim.dialog}
          onChange={(dialog) => setAnim({ dialog })}
          options={DIALOG_OPTIONS}
        />
      </Row>
      <Row
        title="Notices"
        hint="How the small messages at the bottom, like “Archived. Undo”, appear."
      >
        <div className="flex items-center gap-2">
          <Button onClick={() => toast('This is how a notice appears.')}>Try it</Button>
          <Select
            label="Notices"
            className="w-44"
            align="end"
            value={anim.toast}
            onChange={(t) => setAnim({ toast: t })}
            options={TOAST_OPTIONS}
          />
        </div>
      </Row>
      <Row title="Sent postmark" hint="The stamp that lands on a reply when you send it.">
        <Select
          label="Sent postmark"
          className="w-44"
          align="end"
          value={anim.stamp}
          onChange={(stamp) => setAnim({ stamp })}
          options={STAMP_OPTIONS}
        />
      </Row>
      <Row title="Strand drawing" hint="The line of a conversation drawing itself as you open it.">
        <Toggle
          label="Strand drawing"
          checked={anim.strand}
          onChange={(strand) => setAnim({ strand })}
        />
      </Row>
      <Row title="Sorting flights" hint="A card travelling to where it was sorted in New senders.">
        <Toggle
          label="Sorting flights"
          checked={anim.flight}
          onChange={(flight) => setAnim({ flight })}
        />
      </Row>
      <div className="py-4">
        <Button onClick={() => setAnim({ ...DEFAULT_ANIMATIONS })}>Back to the defaults</Button>
      </div>
    </>
  );
}
