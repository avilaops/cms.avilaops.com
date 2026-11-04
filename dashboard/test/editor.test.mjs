import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, mkdir } from 'node:fs/promises';
import { chromium } from 'playwright';

// These pages are produced by the authenticated Rust router tests, not a
// duplicate of the editor markup. The server here only replays those artifacts.
test('editor real: navegação, dados, prévia, teclado e celular', async () => {
  const capture = new URL('../../target/editor-capture/', import.meta.url);
  const editor = await readFile(new URL('editor.html', capture));
  const preview = await readFile(new URL('preview.html', capture));
  const script = await readFile(new URL('../dist/editor.js', import.meta.url));
  const style = await readFile(new URL('../dist/editor.css', import.meta.url));
  let submitted;
  const server = createServer(async (req, res) => {
    if (req.method === 'POST') {
      let body = '';
      for await (const chunk of req) body += chunk;
      submitted = new URLSearchParams(body);
      res.writeHead(200, { 'Content-Type': 'text/html' });
      return res.end('<p>Recebido pelo servidor de teste</p>');
    }
    if (/\/editor-[a-f0-9]+\.js$/.test(req.url)) {
      res.setHeader('Content-Type', 'text/javascript'); return res.end(script);
    }
    if (/\/editor-[a-f0-9]+\.css$/.test(req.url)) {
      res.setHeader('Content-Type', 'text/css'); return res.end(style);
    }
    if (req.url.endsWith('.woff2')) { res.writeHead(404); return res.end(); }
    res.setHeader('Content-Type', 'text/html');
    res.end(req.url.endsWith('/previa') ? preview : editor);
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || '/usr/bin/google-chrome', args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const url = `http://127.0.0.1:${server.address().port}/editor`;
    await page.goto(url);
    await page.waitForSelector('.editor-ready');
    const fields = await page.locator('#editor-form').evaluate(form => [...new FormData(form)]);
    await page.getByRole('button', { name: /Parágrafo/ }).click();
    assert.equal(await page.locator('#editor-properties').isVisible(), false);
    assert.deepEqual(await page.locator('#editor-form').evaluate(form => [...new FormData(form)]), fields);
    await page.getByRole('button', { name: /Identidade e abertura/ }).click();
    await page.locator('#titulo').fill('Título editado no navegador');
    await page.getByRole('status').filter({ hasText: 'Alterações não salvas' }).waitFor();
    await page.getByRole('button', { name: 'Ver prévia salva' }).click();
    await page.getByText('Salve o rascunho para incluir suas alterações na prévia.').waitFor();
    await page.frameLocator('iframe').getByRole('heading', { name: 'Sobre a padaria' }).waitFor();
    await page.getByRole('button', { name: 'Ver celular', exact: true }).click();
    assert.equal(await page.locator('iframe').evaluate(e => e.getBoundingClientRect().width), 390);
    await page.screenshot({ path: new URL('desktop.png', capture).pathname, fullPage: true });
    await page.getByRole('button', { name: 'Fechar prévia' }).click();
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await page.screenshot({ path: new URL('mobile.png', capture).pathname, fullPage: true });
    await page.keyboard.press('Control+s');
    await page.getByText('Recebido pelo servidor de teste').waitFor();
    assert.equal(submitted.get('acao'), 'salvar');
    assert.equal(submitted.get('titulo'), 'Título editado no navegador');
    assert.equal(submitted.get('b0_texto'), new Map(fields).get('b0_texto'));
    assert.deepEqual(errors, []);
    const fallback = await browser.newPage({ javaScriptEnabled: false });
    await fallback.goto(url);
    assert.equal(await fallback.locator('#titulo').isVisible(), true);
    assert.equal(await fallback.locator('[name=b0_texto]').isVisible(), true);
    assert.equal(await fallback.getByRole('button', { name: 'Salvar rascunho', exact: true }).count(), 1);
  } finally {
    await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
});
