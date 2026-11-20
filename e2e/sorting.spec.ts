import { expect, type Page, test } from '@playwright/test';

test.describe.configure({ mode: 'serial' });

const NOW = '2026-11-18T10:00:00+01:00';
const url = (hash: string) => `/?now=${encodeURIComponent(NOW)}#${hash}`;

test.use({ viewport: { width: 1360, height: 860 } });

async function ipc<T>(page: Page, cmd: string, data: object = {}): Promise<T> {
  const response = await page.request.post(`/ipc/${cmd}`, { data });
  return (await response.json()) as T;
}

async function personId(page: Page, name: string): Promise<number> {
  const people = await ipc<{ id: number }[]>(page, 'list_people', { filter: 'all', query: name });
  if (!people[0]) throw new Error(`no demo person matches ${name}`);
  return people[0].id;
}

function watchForErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error' && !m.text().includes('Failed to load resource'))
      errors.push(`console: ${m.text()}`);
  });
  page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
  return errors;
}

async function waitingNames(page: Page): Promise<string[]> {
  const senders = await ipc<{ person: { name: string } }[]>(page, 'list_new_senders');
  return senders.map((s) => s.person.name);
}

test('the New senders deck: a key decides, the toast offers Undo, and Z brings the card back', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/new-senders'));
  const deck = page.getByRole('region', { name: 'New senders' });
  await expect(deck.getByRole('heading', { level: 1, name: 'New senders' })).toBeVisible();
  const before = await waitingNames(page);
  expect(before).toHaveLength(5);
  await expect(page.getByRole('link', { name: 'New senders' })).toContainText('5');

  const top = before[0] as string;
  await expect(deck.getByRole('article', { name: `${top}, new sender` })).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/new-senders.png' });

  await page.keyboard.press('a');
  await expect(page.getByRole('status').filter({ hasText: `${top} allowed.` })).toBeVisible();
  await expect.poll(() => waitingNames(page)).toHaveLength(4);
  const people = await ipc<{ name: string }[]>(page, 'list_people', {
    filter: 'all',
    query: top.split(' ')[0],
  });
  expect(people.map((p) => p.name)).toContain(top);
  await expect(page.getByRole('link', { name: 'New senders' })).toContainText('4');

  await page.getByRole('button', { name: 'Undo' }).click();
  await expect.poll(() => waitingNames(page)).toHaveLength(5);
  await expect(deck.getByRole('article', { name: `${top}, new sender` })).toBeVisible();

  await page.keyboard.press('h');
  await expect.poll(() => waitingNames(page)).toHaveLength(4);
  await page.keyboard.press('z');
  await expect(page.getByRole('status').filter({ hasText: /Undone: .* hidden\./ })).toBeVisible();
  await expect.poll(() => waitingNames(page)).toHaveLength(5);
  expect(errors).toEqual([]);
});

test('the buttons decide too, and blocking sends someone to the Hidden screen as blocked', async ({
  page,
}) => {
  await page.goto(url('/new-senders'));
  const first = (await waitingNames(page))[0] as string;
  await expect(page.getByRole('article', { name: `${first}, new sender` })).toBeVisible();
  await page.getByRole('button', { name: /^Block/ }).click();
  await expect.poll(() => waitingNames(page)).toHaveLength(4);

  await page.goto(url('/hidden'));
  const blocked = page.getByRole('region', { name: 'Blocked senders' });
  await expect(blocked.getByText(first)).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/hidden.png' });
  await blocked.getByRole('button', { name: 'Unblock' }).first().click();
  await expect.poll(() => waitingNames(page)).toHaveLength(4);
  await expect(page.getByText(`${first} is back.`)).toBeVisible();
  await page.getByRole('button', { name: 'Undo' }).last().click();
  await expect.poll(() => waitingNames(page)).toHaveLength(4);
  await ipc(page, 'undo');
  await expect.poll(() => waitingNames(page)).toHaveLength(5);
});

test('an empty deck says so, and a domain can be decided at once', async ({ page }) => {
  await page.goto(url('/new-senders'));
  const names = await waitingNames(page);
  expect(names).toHaveLength(5);
  await page.getByRole('button', { name: /More choices for/ }).click();
  await page.getByRole('menuitem', { name: /^Hide everyone at/ }).click();
  await expect.poll(() => waitingNames(page)).toHaveLength(4);
  await page.getByRole('button', { name: 'Undo' }).click();
  await expect.poll(() => waitingNames(page)).toHaveLength(5);
  const rules = await ipc<unknown[]>(page, 'list_domain_rules');
  expect(rules, 'undo removed the rule that came with it').toEqual([]);
});

test('archive, trash and hide work from the keyboard and every one can be undone', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  const mateo = await personId(page, 'Mateo');
  const convos = await ipc<{ id: number; subject: string }[]>(page, 'list_conversations', {
    personId: mateo,
  });
  expect(convos).toHaveLength(1);
  const convo = convos[0] as { id: number; subject: string };
  await page.goto(url(`/people/${mateo}/c/${convo.id}`));
  await expect(page.getByRole('heading', { level: 2, name: convo.subject })).toBeVisible();
  await page
    .getByRole('region', { name: 'Conversation', exact: true })
    .click({ position: { x: 5, y: 5 } });

  const inbox = async () =>
    ipc<{ in_inbox?: boolean }>(page, 'get_conversation', { id: convo.id, viewerPersonId: mateo });

  await page.keyboard.press('e');
  await expect(page.getByRole('status').filter({ hasText: 'Archived.' })).toBeVisible();
  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(page.getByText('Undone: Archived.')).toBeVisible();
  void inbox;

  await page.keyboard.press('#');
  await expect(page.getByRole('status').filter({ hasText: 'Moved to trash.' })).toBeVisible();
  await expect
    .poll(
      async () => (await ipc<unknown[]>(page, 'list_conversations', { personId: mateo })).length,
    )
    .toBe(0);
  await page.keyboard.press('Control+z');
  await expect(page.getByText('Undone: Moved to trash.')).toBeVisible();
  await expect
    .poll(
      async () => (await ipc<unknown[]>(page, 'list_conversations', { personId: mateo })).length,
    )
    .toBe(1);

  await page.goto(url(`/people/${mateo}/c/${convo.id}`));
  await expect(page.getByRole('heading', { level: 2, name: convo.subject })).toBeVisible();
  await page.getByRole('button', { name: 'Hide conversation' }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Conversation hidden.' })).toBeVisible();
  await page.goto(url('/hidden'));
  const hidden = page.getByRole('region', { name: 'Conversations you hid' });
  await expect(hidden.getByText(convo.subject)).toBeVisible();
  await hidden.getByRole('button', { name: 'Show again' }).click();
  await expect(page.getByText('Conversation shown.')).toBeVisible();
  await expect
    .poll(
      async () => (await ipc<unknown[]>(page, 'list_conversations', { personId: mateo })).length,
    )
    .toBe(1);
  expect(errors).toEqual([]);
});

test('a person can be hidden from their menu, then brought back from Hidden', async ({ page }) => {
  const hanna = await personId(page, 'Hanna');
  await page.goto(url(`/people/${hanna}`));
  await expect(page.getByRole('heading', { level: 2, name: 'Hanna Berg' })).toBeVisible();
  await page.getByRole('button', { name: 'More about this person' }).click();
  await page.screenshot({ path: 'test-results/screens/person-menu.png' });
  await page.getByRole('menuitem', { name: 'Hide Hanna' }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Hanna Berg hidden.' })).toBeVisible();
  await expect(page).toHaveURL(/#\/people$/);
  await expect(page.getByRole('option', { name: /Hanna Berg/ })).toHaveCount(0);
  expect(
    (
      await ipc<{ people: unknown[]; messages: unknown[] }>(page, 'search', {
        query: 'dentist reminder',
      })
    ).messages,
  ).toHaveLength(0);

  await page.goto(url('/hidden'));
  const section = page.getByRole('region', { name: 'People you hid' });
  await expect(section.getByText('Hanna Berg')).toBeVisible();
  await section.getByRole('button', { name: 'Unhide' }).click();
  await expect(page.getByText('Hanna Berg is back.')).toBeVisible();
  await expect
    .poll(
      async () =>
        (await ipc<unknown[]>(page, 'list_people', { filter: 'all', query: 'Hanna' })).length,
    )
    .toBe(1);
});

test('favourites and muting are remembered, and Settings holds domain rules and groups', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  const wei = await personId(page, 'Wei');
  await page.goto(url(`/people/${wei}`));
  await page.getByRole('button', { name: 'More about this person' }).click();
  await page.getByRole('menuitem', { name: 'Add to favourites' }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Saved.' })).toBeVisible();
  expect((await ipc<{ vip: boolean }>(page, 'get_person_flags', { id: wei })).vip).toBe(true);
  await page.keyboard.press('Control+z');
  await expect
    .poll(async () => (await ipc<{ vip: boolean }>(page, 'get_person_flags', { id: wei })).vip)
    .toBe(false);

  await page.goto(url('/settings'));
  await page.getByLabel('Domain').fill('spam-shop.example');
  await page.getByRole('combobox', { name: 'What to do' }).click();
  await page.getByRole('option', { name: /^Block/ }).click();
  await page.getByRole('button', { name: 'Add rule' }).click();
  const rules = page.getByRole('list', { name: 'Domain rules' });
  await expect(rules.getByText('spam-shop.example')).toBeVisible();
  await rules.getByRole('button', { name: 'Remove' }).click();
  await expect(rules).toHaveCount(0);

  await page.getByLabel('Group name').fill('Neighbours');
  await page.getByRole('button', { name: 'Add group' }).click();
  await expect(page.getByRole('list', { name: 'Groups' }).getByText('Neighbours')).toBeVisible();

  await page.goto(url(`/people/${wei}`));
  await page.getByRole('button', { name: 'Edit person' }).click();
  const dialog = page.getByRole('dialog', { name: /Edit Wei/ });
  await dialog.getByText('Neighbours').click();
  await dialog.getByRole('button', { name: 'Save' }).click();
  await expect(dialog).toBeHidden();
  const found = await ipc<{ messages: unknown[] }>(page, 'search', { query: 'group:neighbours' });
  expect(found.messages.length).toBeGreaterThan(0);

  await page.goto(url('/settings'));
  await page.getByRole('list', { name: 'Groups' }).getByRole('button', { name: 'Delete' }).click();
  await expect(page.getByRole('list', { name: 'Groups' })).toHaveCount(0);
  expect(errors).toEqual([]);
});
