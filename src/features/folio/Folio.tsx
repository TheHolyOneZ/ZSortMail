import { useNavigate, useParams } from '@tanstack/react-router';
import { type RefObject, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useConversations, usePeople } from '@/lib/queries';
import { useActions } from '@/lib/shortcuts';
import { PANE_LIMITS, useUi } from '@/lib/store';
import { FoldStack } from '@/motion/choreographies/fold';
import { Splitter } from '@/ui/Splitter';
import { ConversationsPane } from '../conversations/ConversationsPane';
import { PeoplePane } from '../people/PeoplePane';
import { StrandView } from '../strand/StrandView';

const STRAND_MIN_PX = 500;

function useElementWidth(ref: RefObject<HTMLElement | null>): number {
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setWidth(el.getBoundingClientRect().width);
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setWidth(entry.contentRect.width);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref]);
  return width;
}

export function Folio() {
  const params = useParams({ strict: false });
  const personId = params.personId ? Number(params.personId) : undefined;
  const conversationId = params.conversationId ? Number(params.conversationId) : undefined;

  const ref = useRef<HTMLDivElement>(null);
  const width = useElementWidth(ref);
  const panes = useUi((u) => u.paneWidths);
  const setPane = useUi((u) => u.setPaneWidth);
  const resetPane = useUi((u) => u.resetPaneWidth);
  const wide = width >= panes.people + panes.conversations + STRAND_MIN_PX;
  const navigate = useNavigate();
  const { data: conversations } = useConversations(personId);
  const filter = useUi((u) => u.peopleFilter);
  const query = useUi((u) => u.peopleQuery);
  const { data: people } = usePeople(filter, query);

  const step = <T,>(list: T[] | undefined, at: number, by: number): T | undefined =>
    list && list.length > 0 ? list[Math.min(list.length - 1, Math.max(0, at + by))] : undefined;
  const goConversation = (by: number) => {
    const at = conversations?.findIndex((c) => c.id === conversationId) ?? -1;
    const target = step(conversations, at < 0 ? (by > 0 ? -1 : 0) : at, by);
    if (target && personId !== undefined)
      void navigate({
        to: '/people/$personId/c/$conversationId',
        params: { personId: String(personId), conversationId: String(target.id) },
      });
  };
  const goPerson = (by: number) => {
    const at = people?.findIndex((p) => p.id === personId) ?? -1;
    const target = step(people, at < 0 ? (by > 0 ? -1 : 0) : at, by);
    if (target) void navigate({ to: '/people/$personId', params: { personId: String(target.id) } });
  };
  useActions({
    next_conversation: () => goConversation(1),
    previous_conversation: () => goConversation(-1),
    next_person: () => goPerson(1),
    previous_person: () => goPerson(-1),
  });

  const latest = conversations?.[0];
  useEffect(() => {
    if (wide && personId !== undefined && conversationId === undefined && latest) {
      navigate({
        to: '/people/$personId/c/$conversationId',
        params: { personId: String(personId), conversationId: String(latest.id) },
        replace: true,
      });
    }
  }, [wide, personId, conversationId, latest, navigate]);

  return (
    <div ref={ref} className="relative h-full">
      {width === 0 ? null : wide ? (
        <div
          className="relative grid h-full"
          style={{
            gridTemplateColumns: `${panes.people}px ${panes.conversations}px minmax(0, 1fr)`,
            gridTemplateRows: 'minmax(0, 1fr)',
          }}
        >
          <div className="min-w-0 border-r border-rule">
            <PeoplePane selectedId={personId} />
          </div>
          <div className="min-w-0 border-r border-rule">
            <ConversationsPane personId={personId} selectedId={conversationId} />
          </div>
          <div className="min-w-0">
            <StrandView conversationId={conversationId} personId={personId} />
          </div>
          <Splitter
            label="Resize the People list"
            value={panes.people}
            min={PANE_LIMITS.people.min}
            max={Math.min(PANE_LIMITS.people.max, width - panes.conversations - STRAND_MIN_PX)}
            onChange={(w) => setPane('people', w)}
            onDefault={() => resetPane('people')}
            style={{ left: panes.people - 4 }}
          />
          <Splitter
            label="Resize the Conversations list"
            value={panes.conversations}
            min={PANE_LIMITS.conversations.min}
            max={Math.min(PANE_LIMITS.conversations.max, width - panes.people - STRAND_MIN_PX)}
            onChange={(w) => setPane('conversations', w)}
            onDefault={() => resetPane('conversations')}
            style={{ left: panes.people + panes.conversations - 4 }}
          />
        </div>
      ) : (
        <FoldStack
          level={conversationId !== undefined ? 2 : personId !== undefined ? 1 : 0}
          panes={[
            <PeoplePane key="people" selectedId={personId} />,
            <ConversationsPane
              key="conversations"
              personId={personId}
              selectedId={conversationId}
              showBack
            />,
            <StrandView
              key="strand"
              conversationId={conversationId}
              personId={personId}
              onBack={() =>
                personId !== undefined &&
                navigate({ to: '/people/$personId', params: { personId: String(personId) } })
              }
            />,
          ]}
        />
      )}
    </div>
  );
}
