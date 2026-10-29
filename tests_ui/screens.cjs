// The README and site screenshots (1440×900, docs/images/) from the built window with the real
// engine behind it (the test server). Run: cargo build -p datum-core --example serve &&
// npm run build:ui && node tests_ui/screens.cjs
const { chromium } = require("playwright");
const { spawn } = require("child_process");
const path = require("path");
const root = path.join(__dirname, "..");
const out = path.join(root, "docs/images");
const exe = path.join(root, "target/debug/examples", process.platform === "win32" ? "serve.exe" : "serve");
const port = 8900 + Math.floor(Math.random() * 90);

(async () => {
  const srv = spawn(exe, [String(port)], { stdio: ["ignore", "pipe", "ignore"] });
  await new Promise(ok => srv.stdout.on("data", d => String(d).includes("ready") && ok()));
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  p.setDefaultTimeout(8000);
  await p.addInitScript(() => {
    window.__TAURI__ = { core: { invoke: async (cmd, args) => { const r = await fetch("/invoke", { method: "POST", body: JSON.stringify({ cmd, args: args ?? {} }) }); const j = await r.json(); if ("err" in j) throw j.err; return j.ok; } }, event: { listen: async () => () => {} } };
  });
  await p.goto(`http://127.0.0.1:${port}/`);
  await p.waitForTimeout(900);
  const open = async name => {
    await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(200);
    await p.click(`.tile:has(b:text-is("${name}"))`); await p.waitForTimeout(300);
    const d = await p.$('.dlg button:has-text("Don\'t save")'); if (d) { await d.click(); await p.waitForTimeout(300); }
    await p.waitForTimeout(800);
  };
  const scr = (x, y) => p.evaluate(([x, y]) => window.__toScreen(x, y), [x, y]);

  // 1. The compressor package, with the anti-surge controller selected.
  await open("Compressor package");
  await p.click(".bcollapse"); await p.waitForTimeout(300);
  await p.keyboard.press("f"); await p.waitForTimeout(300);
  const [ux, uy] = await scr(165, 30); await p.mouse.click(ux, uy); await p.waitForTimeout(300);
  await p.screenshot({ path: `${out}/compressor.png` });

  // 2. The Checks panel on Find the faults.
  await p.click(".bcollapse"); await open("Find the faults");
  await p.click(".ctbl tr:has(td:text-is('8'))"); await p.waitForTimeout(400);
  await p.keyboard.press("f"); await p.waitForTimeout(300);
  await p.screenshot({ path: `${out}/checks.png` });

  // 3. The line list with hydraulics, a line selected with its hydraulics in the Inspector.
  await open("Pump transfer");
  await p.click(".btabs button:has-text('Line list')"); await p.waitForTimeout(500);
  await p.keyboard.press("f"); await p.waitForTimeout(300);
  const [lx, ly] = await scr(232, 134); await p.mouse.click(lx, ly); await p.waitForTimeout(500);
  await p.evaluate(() => { const s = document.querySelector(".lscroll"); if (s) s.scrollLeft = 260; });
  await p.waitForTimeout(200);
  await p.screenshot({ path: `${out}/line-list.png` });

  // 4. A printed sheet (the print preview).
  await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(200);
  await p.click(".bs aside button:has-text('Print')"); await p.waitForTimeout(1200);
  await p.fill(".pzoom input", "0.6").catch(() => {});
  await p.evaluate(() => { const r = document.querySelector(".pzoom input"); r.value = "0.62"; r.dispatchEvent(new Event("input", { bubbles: true })); });
  await p.waitForTimeout(400);
  await p.screenshot({ path: `${out}/print.png` });

  await b.close();
  srv.kill();
  console.log("Saved compressor.png, checks.png, line-list.png and print.png in docs/images");
})().catch(e => { console.error(e); process.exit(1); });
