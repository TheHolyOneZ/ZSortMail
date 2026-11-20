import { useEffect, useRef, useState } from 'react';
import { api } from '@/lib/api';
import { usePerson } from '@/lib/queries';
import { toastError } from '@/lib/toasts';

export function NotesTab({ personId }: { personId: number }) {
  const { data: person } = usePerson(personId);
  const saved = person?.notes ?? '';
  const [text, setText] = useState(saved);
  const [state, setState] = useState<'idle' | 'saving' | 'saved'>('idle');
  const lastSaved = useRef(saved);
  const loadedFor = useRef<number | null>(null);

  useEffect(() => {
    if (person && loadedFor.current !== personId) {
      loadedFor.current = personId;
      lastSaved.current = person.notes ?? '';
      setText(person.notes ?? '');
    }
  }, [person, personId]);

  useEffect(() => {
    if (loadedFor.current !== personId || text === lastSaved.current) return;
    setState('saving');
    const timer = setTimeout(() => {
      api
        .setPersonNotes(personId, text)
        .then(() => {
          lastSaved.current = text.trim();
          setState('saved');
        })
        .catch((e) => {
          setState('idle');
          toastError(e);
        });
    }, 700);
    return () => clearTimeout(timer);
  }, [text, personId]);

  return (
    <div className="p-4">
      <label className="block">
        <span className="sr-only">Notes about this person</span>
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="How you know them, what they prefer, what to remember."
          rows={10}
          dir="auto"
          className="block w-full resize-y rounded-sheet bg-sheet p-3 text-body leading-relaxed text-text shadow-sheet outline-none placeholder:text-text-2 focus-visible:outline-2 focus-visible:outline-blue"
        />
      </label>
      <p className="mt-2 h-4 text-meta text-text-2" aria-live="polite">
        {state === 'saving' ? 'Saving' : state === 'saved' ? 'Saved' : 'Only you can see this.'}
      </p>
    </div>
  );
}
