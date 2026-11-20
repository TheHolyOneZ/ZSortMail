import { accountLabel } from '@/lib/accounts';
import { cn } from '@/lib/cn';
import { profileKey } from '@/lib/profiles';
import { useAppState } from '@/lib/queries';
import { useUi } from '@/lib/store';
import { Select } from '@/ui/Select';

export function ProfileSwitcher({ open }: { open: boolean }) {
  const accounts = useAppState().data?.accounts ?? [];
  const active = useUi((s) => s.profiles.active);
  const switchProfile = useUi((s) => s.switchProfile);
  if (accounts.length < 2) return null;
  const options = accounts.map((a) => ({
    value: profileKey(a.email),
    label: accountLabel(a),
  }));
  const current = options.find((o) => o.value === active) ?? options[0];
  if (!current) return null;

  if (!open) {
    const next =
      options[(options.findIndex((o) => o.value === current.value) + 1) % options.length];
    return (
      <button
        type="button"
        title={`Profile: ${current.label}. Switch to ${next?.label}.`}
        aria-label={`Profile ${current.label}. Switch profile`}
        onClick={() => next && switchProfile(next.value)}
        className={cn(
          'mb-3 grid size-[32px] place-items-center rounded-full bg-text text-meta font-semibold text-sheet',
        )}
      >
        {current.label.slice(0, 1).toUpperCase()}
      </button>
    );
  }
  return (
    <Select
      label="Profile"
      size="sm"
      className="mb-3 w-full"
      value={current.value}
      onChange={switchProfile}
      options={options}
    />
  );
}
