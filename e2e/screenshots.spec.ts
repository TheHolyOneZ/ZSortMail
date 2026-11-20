import { expect, type Page, test } from '@playwright/test';

test.skip(!process.env.ZSM_SHOTS, 'only when asked for');
test.describe.configure({ mode: 'serial' });
test.use({ viewport: { width: 1440, height: 900 } });

const NOW = '2026-11-18T10:00:00+01:00';
const url = (hash: string) => `/?now=${encodeURIComponent(NOW)}#${hash}`;
const out = (name: string) => `screenshots/${name}.png`;

async function ipc<T>(page: Page, cmd: string, args: object = {}): Promise<T> {
  const r = await page.request.post(`/ipc/${cmd}`, { data: args });
  return (await r.json()) as T;
}

async function settle(page: Page, ms = 900) {
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(ms);
}

async function look(page: Page, theme: string) {
  await page.addInitScript((t) => {
    localStorage.setItem(
      'zsm-ui',
      JSON.stringify({ state: { theme: t, categoriesFirst: false, motion: 'off' }, version: 0 }),
    );
  }, theme);
}

test('a few categories exist for the pictures', async ({ page }) => {
  await page.goto(url('/people'));
  const find = async (q: string) =>
    (await ipc<{ id: number }[]>(page, 'list_people', { filter: 'all', query: q }))[0]
      ?.id as number;
  const make = (name: string, icon: string, ink: number, description: string) =>
    ipc<number>(page, 'save_category', {
      input: { id: null, name, ink, icon, description, image: null, sidebar: false },
    });
  const family = await make('Family', 'house', 1, 'The people at home');
  const work = await make('Work', 'briefcase', 6, 'Launch team and the accountant');
  const health = await make('Health', 'stethoscope', 4, 'Doctors and the dentist');
  await ipc(page, 'add_to_category', {
    group: family,
    person: await find('Dad'),
    email: null,
    name: null,
  });
  for (const q of ['Okafor', 'Whitaker', 'Rasmus'])
    await ipc(page, 'add_to_category', {
      group: work,
      person: await find(q),
      email: null,
      name: null,
    });
  await ipc(page, 'add_to_category', {
    group: health,
    person: await find('Hanna'),
    email: null,
    name: 'Dr. Berg',
  });
  const maya = await find('Okafor');
  await ipc(page, 'set_person_flags', {
    id: maya,
    flags: { whitelisted: false, vip: true, muted: false },
  });
});

test('people, conversations and the strand', async ({ page }) => {
  await look(page, 'light');
  await page.goto(url('/people'));
  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await settle(page, 1400);
  await page.screenshot({ path: out('people-light') });
});

test('the same in the dark theme', async ({ page }) => {
  await look(page, 'dark');
  await page.goto(url('/people'));
  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await settle(page, 1400);
  await page.screenshot({ path: out('people-dark') });
});

test('categories', async ({ page }) => {
  await look(page, 'light');
  await page.addInitScript(() =>
    localStorage.setItem(
      'zsm-ui',
      JSON.stringify({ state: { theme: 'light', motion: 'off' }, version: 0 }),
    ),
  );
  await page.goto(url('/people'));
  await expect(page.getByRole('list', { name: 'Categories' })).toBeVisible();
  await settle(page);
  await page.screenshot({ path: out('categories') });
});

test('mail with the message beside the list', async ({ page }) => {
  await look(page, 'light');
  await page.goto(url('/river'));
  await page
    .getByRole('list', { name: 'Conversations' })
    .getByRole('button', { name: /Q3 launch checklist/ })
    .first()
    .click();
  await settle(page, 1400);
  await page.screenshot({ path: out('mail') });
});

test('waiting for', async ({ page }) => {
  await look(page, 'light');
  await ipc(page, 'save_watch', {
    input: {
      id: null,
      name: 'xbox 394',
      query: 'xbox 394',
      mode: 'once',
      priority: 'normal',
      expires_at: null,
      include_existing: true,
    },
  });
  await ipc(page, 'save_watch', {
    input: {
      id: null,
      name: 'passport appointment',
      query: 'passport appointment',
      mode: 'once',
      priority: 'urgent',
      expires_at: null,
      include_existing: false,
    },
  });
  await page.goto(url('/waiting'));
  await settle(page);
  await page.screenshot({ path: out('waiting') });
});

test('rules with a preview', async ({ page }) => {
  await look(page, 'light');
  await page.goto(url('/rules'));
  await page.getByRole('button', { name: 'New rule' }).click();
  const dialog = page.getByRole('dialog', { name: 'New rule' });
  await dialog.getByLabel('When mail matches').fill('category:newsletter');
  await expect(dialog.getByText(/This would catch/)).toBeVisible();
  await dialog.getByLabel(/^Archive/).check({ force: true });
  await dialog.getByLabel('Mark as read').check({ force: true });
  await settle(page, 700);
  await page.screenshot({ path: out('rules') });
});

test('the composer', async ({ page }) => {
  await look(page, 'light');
  await page.goto(url('/people'));
  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await page.getByRole('button', { name: /^Write to Maya/ }).click();
  const composer = page.getByRole('region', { name: 'New message' });
  await composer.getByLabel('Subject').fill('Thursday review');
  await composer.getByRole('textbox', { name: 'Message' }).click();
  await page.keyboard.type(
    'Thursday at 10 works for me. I will bring the updated checklist and the screenshots from Dan.',
  );
  await settle(page, 700);
  await page.screenshot({ path: out('composer') });
});

test('routines', async ({ page }) => {
  await look(page, 'light');
  await page.goto(url('/routines'));
  await page.getByRole('button', { name: 'New routine' }).click();
  const dialog = page.getByRole('dialog', { name: 'New routine' });
  await dialog.getByLabel('Name').fill('Monday report');
  await dialog.getByLabel('To', { exact: true }).fill('boss@elsewhere.example');
  await page.keyboard.press('Enter');
  await dialog.getByLabel('Subject').fill('Report for {{weekday}}');
  await dialog.getByLabel('Text').fill('Numbers for {{date}}.');
  await dialog.getByRole('button', { name: 'Save routine' }).click();
  await settle(page);
  await page.screenshot({ path: out('routines') });
});

test('new senders', async ({ page }) => {
  await look(page, 'light');
  await page.goto(url('/new-senders'));
  await settle(page, 1200);
  await page.screenshot({ path: out('new-senders') });
});

test('the wall of people', async ({ page }) => {
  await look(page, 'light');
  await page.addInitScript(() =>
    localStorage.setItem(
      'zsm-ui',
      JSON.stringify({
        state: { theme: 'light', categoriesFirst: false, motion: 'off', peopleView: 'wall' },
        version: 0,
      }),
    ),
  );
  await page.goto(url('/people'));
  await settle(page, 1200);
  await page.screenshot({ path: out('wall') });
});

test('themes and the theme editor', async ({ page }) => {
  await look(page, 'light');
  await page.goto(url('/settings'));
  await page.getByRole('button', { name: 'Create a theme' }).click();
  const dialog = page.getByRole('dialog', { name: 'New theme' });
  await dialog.getByLabel('Name').fill('Dusk');
  await dialog.getByRole('textbox', { name: 'Accent as hex or rgb', exact: true }).fill('#ff7800');
  await dialog
    .getByRole('textbox', { name: 'Background as hex or rgb', exact: true })
    .fill('#0a1428');
  await dialog.getByRole('textbox', { name: 'Text as hex or rgb', exact: true }).fill('#e5ecf3');
  await settle(page, 900);
  await page.screenshot({ path: out('theme-editor') });
});

test('the Google setup guide', async ({ page }) => {
  await look(page, 'light');
  await page.goto(url('/settings'));
  await page.getByRole('button', { name: 'Set up Google access' }).first().click();
  const guide = page.getByRole('dialog', { name: 'Set up Google access' });
  for (let i = 0; i < 4; i++) await guide.getByRole('button', { name: 'Done, next step' }).click();
  await settle(page, 700);
  await page.screenshot({ path: out('google-guide') });
});
