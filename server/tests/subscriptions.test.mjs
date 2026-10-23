import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { PGlite } from '@electric-sql/pglite';

test('subscription grants: tiers, expiration, cancellation and overlapping subscriptions', async () => {
  const db = new PGlite();
  try {
    await db.exec('CREATE ROLE kuncat_accounts_runtime');
    await db.exec(readFileSync(new URL('../../../kuncat.com/platform/migrations/001_accounts.sql', import.meta.url), 'utf8'));
    await db.exec(readFileSync(new URL('../../../infra/platform/subscriptions/migrations/003_subscriptions.sql', import.meta.url), 'utf8'));
    const { rows: [{ id }] } = await db.query('INSERT INTO kuncat.account DEFAULT VALUES RETURNING id');
    await db.query("INSERT INTO kuncat.billing_customer VALUES ($1,'stripe','cus_test')", [id]);
    const grants = async () => (await db.query('SELECT DISTINCT capability FROM kuncat.subscription_access WHERE account_id=$1 ORDER BY capability', [id])).rows.map(x => x.capability);
    assert.deepEqual(await grants(), []);
    await db.exec("INSERT INTO kuncat.subscription(provider,subscription_id,customer_id,plan,status,current_period_end) VALUES ('stripe','sub_games','cus_test','games','active',now()+interval '1 day')");
    assert.deepEqual(await grants(), ['games:host']);
    await db.exec("UPDATE kuncat.subscription SET plan='all_access',cancel_at_period_end=true");
    assert.deepEqual(await grants(), ['games:host','kunatio:access']);
    for (const status of ['past_due','canceled','unpaid','paused','incomplete','incomplete_expired']) {
      await db.query('UPDATE kuncat.subscription SET status=$1', [status]);
      assert.deepEqual(await grants(), [], status);
    }
    await db.exec("UPDATE kuncat.subscription SET status='trialing'");
    assert.deepEqual(await grants(), ['games:host','kunatio:access']);
    await db.exec("UPDATE kuncat.subscription SET current_period_end=now()-interval '1 second'");
    assert.deepEqual(await grants(), []);
    await db.exec("INSERT INTO kuncat.subscription(provider,subscription_id,customer_id,plan,status,current_period_end) VALUES ('stripe','sub_other','cus_test','games','active',now()+interval '1 day')");
    assert.deepEqual(await grants(), ['games:host']);
    await db.exec('GRANT USAGE ON SCHEMA kuncat TO kuncat_accounts_runtime');
    await db.exec('SET ROLE kuncat_accounts_runtime');
    assert.deepEqual(await grants(), ['games:host']);
    await assert.rejects(db.exec("UPDATE kuncat.subscription SET plan='all_access'"), /permission denied/);
  } finally { await db.close(); }
});
