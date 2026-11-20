import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import { api } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { useDomainRules, useGroups } from '@/lib/queries';
import { toastError } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { Select } from '@/ui/Select';

const field =
  'h-9 rounded-control bg-hover px-3 text-body text-text outline-none placeholder:text-text-2 focus-visible:outline-2 focus-visible:outline-blue';

export function SortingSection() {
  const t = useT();
  return (
    <>
      <h2 className="mt-8 text-name font-semibold">{t('settings.sorting')}</h2>
      <div className="border-b border-rule py-4">
        <h3 className="text-body font-medium">Rules</h3>
        <p className="mt-0.5 max-w-[56ch] text-ui text-text-2">
          Sort new mail the moment it arrives: archive, hide, label, mute and more.{' '}
          <Link to="/rules" className="font-medium text-blue underline underline-offset-2">
            Manage rules
          </Link>
          , or see what you have{' '}
          <Link to="/hidden" className="font-medium text-blue underline underline-offset-2">
            hidden
          </Link>
          .
        </p>
      </div>
      <DomainRules />
      <Groups />
    </>
  );
}

function DomainRules() {
  const { data: rules = [] } = useDomainRules();
  const [domain, setDomain] = useState('');
  const [action, setAction] = useState<'allow' | 'hide' | 'block'>('allow');

  async function add() {
    try {
      await api.setDomainRule(domain, action);
      setDomain('');
    } catch (e) {
      toastError(e);
    }
  }

  return (
    <div className="border-b border-rule py-4">
      <h3 className="text-body font-medium">Domain rules</h3>
      <p className="mt-0.5 max-w-[56ch] text-ui text-text-2">
        Decide in advance what happens to anyone new who writes from a domain. Without a rule, they
        wait in New senders.
      </p>
      {rules.length > 0 && (
        <ul className="mt-3" aria-label="Domain rules">
          {rules.map((r) => (
            <li key={r.domain} className="flex items-center gap-3 py-1.5">
              <span dir="ltr" className="min-w-0 flex-1 truncate text-ui">
                {r.domain}
              </span>
              <span className="w-16 text-ui text-text-2">
                {r.action === 'allow' ? 'Allow' : r.action === 'hide' ? 'Hide' : 'Block'}
              </span>
              <Button onClick={() => void api.setDomainRule(r.domain, null).catch(toastError)}>
                Remove
              </Button>
            </li>
          ))}
        </ul>
      )}
      <form
        className="mt-3 flex flex-wrap items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <label className="sr-only" htmlFor="rule-domain">
          Domain
        </label>
        <input
          id="rule-domain"
          className={`${field} w-56`}
          placeholder="example.com"
          value={domain}
          onChange={(e) => setDomain(e.target.value)}
          dir="ltr"
          autoComplete="off"
          spellCheck={false}
        />
        <Select<'allow' | 'hide' | 'block'>
          label="What to do"
          className="w-36"
          value={action}
          onChange={setAction}
          options={[
            { value: 'allow', label: 'Allow', hint: 'Straight into People' },
            { value: 'hide', label: 'Hide', hint: 'Out of sight' },
            { value: 'block', label: 'Block', hint: 'Mail goes to Spam' },
          ]}
        />
        <Button type="submit" disabled={!domain.trim()}>
          Add rule
        </Button>
      </form>
    </div>
  );
}

function Groups() {
  const { data: groups = [] } = useGroups();
  const [name, setName] = useState('');

  async function add() {
    try {
      await api.createGroup(name, groups.length % 10);
      setName('');
    } catch (e) {
      toastError(e);
    }
  }

  return (
    <div className="border-b border-rule py-4">
      <h3 className="text-body font-medium">Groups</h3>
      <p className="mt-0.5 max-w-[56ch] text-ui text-text-2">
        Put people into groups like Family or Clients. Searches, rules and Waiting for can use them
        with <code>group:</code>. Add someone to a group from their Edit person form.
      </p>
      {groups.length > 0 && (
        <ul className="mt-3" aria-label="Groups">
          {groups.map((g) => (
            <li key={g.id} className="flex items-center gap-3 py-1.5">
              <span dir="auto" className="min-w-0 flex-1 truncate text-ui">
                {g.name}
              </span>
              <span className="text-ui text-text-2">
                {g.members === 1 ? '1 person' : `${g.members} people`}
              </span>
              <Button onClick={() => void api.deleteGroup(g.id).catch(toastError)}>Delete</Button>
            </li>
          ))}
        </ul>
      )}
      <form
        className="mt-3 flex flex-wrap items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <label className="sr-only" htmlFor="group-name">
          Group name
        </label>
        <input
          id="group-name"
          className={`${field} w-56`}
          placeholder="Family"
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoComplete="off"
        />
        <Button type="submit" disabled={!name.trim()}>
          Add group
        </Button>
      </form>
    </div>
  );
}
