import { Trash, UserPlus, X } from '@phosphor-icons/react';
import { useState } from 'react';
import { api } from '@/lib/api';
import type { GroupInfo } from '@/lib/bindings';
import { stampColor } from '@/lib/ink';
import { useCategoryMembers, usePeople } from '@/lib/queries';
import { useUi } from '@/lib/store';
import { toast, toastError } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Field, Modal, Section } from '@/ui/Modal';
import { Stamp } from '@/ui/Stamp';
import { Toggle } from '@/ui/Toggle';
import { CategoryBadge } from './CategoryBadge';
import { CATEGORY_ICONS } from './categoryIcons';

const INKS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

async function pictureFrom(file: File): Promise<string> {
  const bitmap = await createImageBitmap(file);
  const side = Math.min(bitmap.width, bitmap.height);
  const canvas = document.createElement('canvas');
  canvas.width = 128;
  canvas.height = 128;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('This computer cannot resize pictures.');
  ctx.drawImage(
    bitmap,
    (bitmap.width - side) / 2,
    (bitmap.height - side) / 2,
    side,
    side,
    0,
    0,
    128,
    128,
  );
  bitmap.close();
  return canvas.toDataURL('image/png');
}

export function Members({ id }: { id: number }) {
  const { data: members = [] } = useCategoryMembers(id);
  const [email, setEmail] = useState('');
  const [name, setName] = useState('');
  const [search, setSearch] = useState('');
  const { data: found = [] } = usePeople('all', search);
  const inside = new Set(members.map((m) => m.person.id));
  const suggestions = search.trim() ? found.filter((p) => !inside.has(p.id)).slice(0, 5) : [];

  async function addAddress() {
    try {
      await api.addToCategory(id, { email, name });
      setEmail('');
      setName('');
    } catch (e) {
      toastError(e);
    }
  }

  return (
    <>
      <ul
        aria-label="Members"
        className="divide-y divide-rule rounded-control border border-rule-strong"
      >
        {members.length === 0 && (
          <li className="px-3 py-3 text-ui text-text-2">Nobody yet. Add people below.</li>
        )}
        {members.map((m) => (
          <li key={m.person.id} className="flex items-center gap-3 px-3 py-2">
            <Stamp
              name={m.alias ?? m.person.name}
              ink={m.person.ink}
              kind={m.person.kind}
              size={32}
            />
            <div className="min-w-0 flex-1">
              <input
                aria-label={`Name in this category for ${m.person.name}`}
                defaultValue={m.alias ?? ''}
                placeholder={m.person.name}
                maxLength={60}
                className="w-full"
                onBlur={(e) => {
                  if (e.target.value.trim() !== (m.alias ?? ''))
                    void api.setMemberAlias(id, m.person.id, e.target.value).catch(toastError);
                }}
              />
              <span className="block truncate text-meta text-text-2">{m.email}</span>
            </div>
            <button
              type="button"
              aria-label={`Remove ${m.person.name} from this category`}
              onClick={() => void api.removeFromCategory(id, m.person.id).catch(toastError)}
              className="grid size-[36px] place-items-center rounded-control text-text-2 hover:bg-hover hover:text-red"
            >
              <X size={14} />
            </button>
          </li>
        ))}
      </ul>

      <div className="mt-3 grid gap-2">
        <Field label="Find someone you already write to">
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Name or address"
            className="w-full"
          />
        </Field>
        {suggestions.length > 0 && (
          <ul aria-label="People found" className="rounded-control border border-rule-strong">
            {suggestions.map((p) => (
              <li key={p.id}>
                <button
                  type="button"
                  onClick={() => {
                    setSearch('');
                    void api.addToCategory(id, { person: p.id }).catch(toastError);
                  }}
                  className="flex w-full items-center gap-3 px-3 py-2 text-start hover:bg-hover"
                >
                  <Stamp name={p.name} ink={p.ink} kind={p.kind} size={28} />
                  <span className="min-w-0 flex-1 truncate text-ui">{p.name}</span>
                  <span className="text-meta font-medium text-blue">Add</span>
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-[1fr_1fr_auto] sm:items-end">
          <Field label="Or add an email address">
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="dr.berg@clinic.example"
              className="w-full"
            />
          </Field>
          <Field label="Name to show">
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Dr. Berg"
              className="w-full"
            />
          </Field>
          <Button onClick={() => void addAddress()} disabled={!email.trim()}>
            <UserPlus size={14} />
            Add
          </Button>
        </div>
      </div>
    </>
  );
}

export function CategoryEditor({
  editing,
  onClose,
}: {
  editing: GroupInfo | null;
  onClose: () => void;
}) {
  const setCategory = useUi((s) => s.setPeopleCategory);
  const [id, setId] = useState<number | null>(editing?.id ?? null);
  const [name, setName] = useState(editing?.name ?? '');
  const [ink, setInk] = useState(editing?.ink ?? 4);
  const [icon, setIcon] = useState(editing?.icon ?? 'users');
  const [description, setDescription] = useState(editing?.description ?? '');
  const [image, setImage] = useState<string | null>(editing?.image ?? null);
  const [sidebar, setSidebar] = useState(editing?.sidebar ?? false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  async function save(close: boolean) {
    try {
      const saved = await api.saveCategory({
        id,
        name,
        ink,
        icon,
        description,
        image,
        sidebar,
      });
      setId(saved);
      if (close) {
        toast(`Saved the category “${name.trim()}”.`);
        onClose();
      }
    } catch (e) {
      toastError(e);
    }
  }

  async function remove() {
    if (id === null) return;
    try {
      await api.deleteGroup(id);
      setCategory('gallery');
      toast(`Deleted the category “${name.trim()}”. The people are still in Everyone.`);
      onClose();
    } catch (e) {
      toastError(e);
    }
  }

  return (
    <Dialog open onClose={onClose} label={editing ? `Edit ${editing.name}` : 'New category'}>
      <Modal
        title={editing ? `Edit ${editing.name}` : 'New category'}
        description="A category is a short list of the people you want to see together, like your doctors or your family."
        onClose={onClose}
        onSubmit={(e) => {
          e.preventDefault();
          void save(true);
        }}
        footer={
          <>
            {id !== null &&
              (confirmDelete ? (
                <Button onClick={() => void remove()} className="me-auto text-red">
                  Delete for good
                </Button>
              ) : (
                <Button onClick={() => setConfirmDelete(true)} className="me-auto">
                  <Trash size={14} />
                  Delete category
                </Button>
              ))}
            <Button onClick={onClose}>Cancel</Button>
            <Button type="submit" variant="primary" disabled={!name.trim()}>
              Save category
            </Button>
          </>
        }
      >
        <div className="flex items-center gap-4">
          <CategoryBadge icon={icon} ink={ink} image={image} size={64} />
          <Field label="Name" className="min-w-0 flex-1">
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              maxLength={40}
              placeholder="Doctors"
              className="w-full"
              autoFocus
            />
          </Field>
        </div>

        <Field label="Description" className="mt-4" hint="Shown under the name. Optional.">
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            maxLength={300}
            rows={2}
            placeholder="People who look after me"
            className="w-full"
          />
        </Field>

        <Section title="Colour">
          <fieldset className="flex flex-wrap gap-2 border-0 p-0">
            <legend className="sr-only">Colour</legend>
            {INKS.map((i) => (
              <label key={i} title={`Colour ${i + 1}`} className="relative cursor-pointer">
                <input
                  type="radio"
                  name="category-colour"
                  checked={ink === i}
                  onChange={() => setInk(i)}
                  aria-label={`Colour ${i + 1}`}
                  className="peer sr-only"
                />
                <span
                  className="block size-[32px] rounded-full border-2 border-transparent peer-checked:border-text peer-checked:shadow-[0_0_0_2px_var(--sheet)_inset] peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-blue"
                  style={{ background: stampColor(i) }}
                />
              </label>
            ))}
          </fieldset>
        </Section>

        <Section title="Icon" hint="Used when there is no picture.">
          <fieldset className="grid grid-cols-6 gap-1.5 border-0 p-0 sm:grid-cols-8">
            <legend className="sr-only">Icon</legend>
            {Object.entries(CATEGORY_ICONS).map(([key, { Icon, label }]) => (
              <label key={key} title={label} className="relative cursor-pointer">
                <input
                  type="radio"
                  name="category-icon"
                  checked={icon === key}
                  onChange={() => setIcon(key)}
                  aria-label={label}
                  className="peer sr-only"
                />
                <span className="grid size-[40px] place-items-center rounded-control border border-rule-strong transition-colors hover:bg-hover peer-checked:border-blue peer-checked:bg-blue-wash peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-blue">
                  <Icon size={20} />
                </span>
              </label>
            ))}
          </fieldset>
        </Section>

        <Section title="Picture" hint="Optional. It is cropped square and kept on this computer.">
          <div className="flex items-center gap-2">
            <label className="btn cursor-pointer rounded-control border border-rule-strong px-3 py-1.5 text-ui hover:bg-hover">
              Choose a picture
              <input
                type="file"
                accept="image/png,image/jpeg,image/webp"
                className="sr-only"
                onChange={(e) => {
                  const file = e.target.files?.[0];
                  if (file) void pictureFrom(file).then(setImage).catch(toastError);
                  e.target.value = '';
                }}
              />
            </label>
            {image && <Button onClick={() => setImage(null)}>Remove picture</Button>}
          </div>
        </Section>

        <Section title="Sidebar">
          <div className="flex items-center justify-between gap-4">
            <span className="text-ui">Show this category in the sidebar</span>
            <Toggle
              checked={sidebar}
              onChange={setSidebar}
              label="Show this category in the sidebar"
            />
          </div>
        </Section>

        <Section title="People in this category">
          {id === null ? (
            <div className="rounded-control border border-dashed border-rule-strong p-4 text-ui text-text-2">
              Save the category first, then add people to it.
              <div className="mt-2">
                <Button onClick={() => void save(false)} disabled={!name.trim()}>
                  Save and add people
                </Button>
              </div>
            </div>
          ) : (
            <Members id={id} />
          )}
        </Section>
      </Modal>
    </Dialog>
  );
}
