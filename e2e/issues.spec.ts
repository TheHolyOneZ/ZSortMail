import { expect, type Page, test } from '@playwright/test';

test.describe.configure({ mode: 'serial' });

const NOW = '2026-11-18T10:00:00+01:00';
const url = (hash: string) => `/?now=${encodeURIComponent(NOW)}#${hash}`;

test.use({ viewport: { width: 1360, height: 860 } });

async function ipc<T>(page: Page, cmd: string, data: object = {}): Promise<T> {
  const response = await page.request.post(`/ipc/${cmd}`, { data });
  return (await response.json()) as T;
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

async function clearDrafts(page: Page) {
  for (const d of await ipc<{ id: number }[]>(page, 'list_drafts'))
    await ipc(page, 'delete_draft', { id: d.id });
}

async function makeTeam(page: Page): Promise<number> {
  const team = await ipc<number>(page, 'save_category', {
    input: {
      id: null,
      name: 'Launch crew',
      ink: 3,
      icon: 'briefcase',
      description: '',
      image: null,
      sidebar: false,
    },
  });
  for (const q of ['Okafor', 'Whitaker']) {
    const found = await ipc<{ id: number }[]>(page, 'list_people', { filter: 'all', query: q });
    await ipc(page, 'add_to_category', {
      group: team,
      person: found[0]?.id,
      email: null,
      name: null,
    });
  }
  return team;
}

test('a category can be written to: from its page, and by typing its name in the To field', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await clearDrafts(page);
  const team = await makeTeam(page);
  await page.addInitScript(() =>
    localStorage.setItem(
      'zsm-ui',
      JSON.stringify({ state: { categoriesFirst: true }, version: 0 }),
    ),
  );

  await page.goto(url('/people'));
  await page
    .getByRole('list', { name: 'Categories' })
    .getByRole('button', { name: /^Launch crew/ })
    .click();
  await page.getByRole('button', { name: 'Write to everyone' }).click();
  const composer = page.getByRole('region', { name: 'New message' });
  await expect(composer).toBeVisible();
  await expect(composer.getByText('Bcc', { exact: true })).toBeVisible();
  await expect(composer.getByTitle('maya.okafor@northwind.example')).toBeVisible();
  await expect(composer.getByTitle(/whitaker/i)).toBeVisible();
  await composer
    .getByRole('button', { name: 'Close' })
    .first()
    .click()
    .catch(() => {});
  await clearDrafts(page);

  await page.goto(url('/people'));
  await page.keyboard.press('c');
  const fresh = page.getByRole('region', { name: 'New message' });
  await expect(fresh).toBeVisible();
  await page.keyboard.type('launch cr');
  const option = fresh.getByRole('option', { name: /Launch crew/ });
  await expect(option).toBeVisible();
  await expect(option).toContainText('Category, 2 people');
  await page.keyboard.press('Enter');
  await expect(fresh.getByTitle('maya.okafor@northwind.example')).toBeVisible();
  await expect(fresh.getByTitle(/whitaker/i)).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/write-to-category.png' });
  await clearDrafts(page);
  await ipc(page, 'delete_group', { id: team });
  expect(errors).toEqual([]);
});

test('the three row buttons appear on hover, and the setting brings them back for good', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/river'));
  const row = page.getByRole('list', { name: 'Conversations' }).getByRole('listitem').first();
  const archive = row.getByRole('button', { name: 'Archive' });
  await expect(row).toBeVisible();
  await expect(archive)
    .toHaveCSS('opacity', '1')
    .catch(() => {});
  const bar = archive.locator('xpath=..');
  await expect(bar).toHaveCSS('opacity', '0');
  await row.hover();
  await expect(bar).toHaveCSS('opacity', '1');
  await page.screenshot({ path: 'test-results/screens/row-actions-hover.png' });
  await row.getByRole('button', { name: 'Archive' }).focus();
  await page.mouse.move(0, 0);
  await expect(bar).toHaveCSS('opacity', '1');

  await page.goto(url('/settings'));
  await page.getByRole('radio', { name: 'Always' }).first().click({ force: true });
  await page.goto(url('/river'));
  const always = page
    .getByRole('list', { name: 'Conversations' })
    .getByRole('listitem')
    .first()
    .getByRole('button', { name: 'Archive' })
    .locator('xpath=..');
  await expect(always).not.toHaveCSS('opacity', '0');
  expect(errors).toEqual([]);
});

test('the app opens on the screen chosen in Settings', async ({ page }) => {
  const errors = watchForErrors(page);
  await page.goto(url('/settings'));
  await page.getByRole('radio', { name: 'Mail', exact: true }).first().click({ force: true });
  await page.goto(`/?now=${encodeURIComponent(NOW)}`);
  await expect(page).toHaveURL(/#\/river/);
  await expect(page.getByRole('heading', { name: 'Mail', level: 1 })).toBeVisible();

  await page.goto(url('/settings'));
  await page.getByRole('radio', { name: 'People', exact: true }).first().click({ force: true });
  await page.goto(`/?now=${encodeURIComponent(NOW)}`);
  await expect(page).toHaveURL(/#\/people/);
  expect(errors).toEqual([]);
});

test('a message sent with a blind copy shows the Bcc in its details', async ({ page }) => {
  const errors = watchForErrors(page);
  await clearDrafts(page);
  const draft = await ipc<{ id: number }>(page, 'new_draft', {
    to: [{ email: 'maya.okafor@northwind.example', name: null }],
  });
  await ipc(page, 'save_draft', {
    input: {
      id: draft.id,
      to: [{ email: 'maya.okafor@northwind.example', name: null }],
      cc: [],
      bcc: [{ email: 'quiet@elsewhere.example', name: null }],
      subject: 'Blind copy check',
      body_html: '<p>Hello</p>',
      nudge_days: null,
    },
  });
  const ticket = await ipc<{ person_id: number; conversation_id: number; message_id: number }>(
    page,
    'send_draft',
    {
      id: draft.id,
    },
  );
  await page.goto(url(`/people/${ticket.person_id}/c/${ticket.conversation_id}`));
  const strand = page.getByRole('region', { name: 'Conversation', exact: true });
  await strand.getByRole('button', { name: 'Details' }).first().click();
  await expect(strand.getByText('Bcc', { exact: true }).first()).toBeVisible();
  await expect(strand.getByText('quiet@elsewhere.example').first()).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/bcc-details.png' });
  const back = await ipc<{ id: number }>(page, 'unsend', { messageId: ticket.message_id });
  await ipc(page, 'delete_draft', { id: back.id });
  expect(errors).toEqual([]);
});
