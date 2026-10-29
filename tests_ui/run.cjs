// UI tests: the built window (ui/dist) in Chromium, driving the real engine through the test server
// (crates/core/examples/serve.rs answers the shell's commands over HTTP). Run: npm run test:ui
// (builds the window and the server, then runs this). SHOTS=dir saves screenshots.
const { chromium } = require("playwright");
const { spawn } = require("child_process");
const path = require("path");
const root = path.join(__dirname, "..");
const exe = path.join(root, "target/debug/examples", process.platform === "win32" ? "serve.exe" : "serve");
const port = 8700 + Math.floor(Math.random() * 200);
let failed = 0;
const check = (name, ok, info = "") => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : "  " + info}`); if (!ok) failed++; };
const shots = process.env.SHOTS;

(async () => {
  const srv = spawn(exe, [String(port)], { stdio: ["ignore", "pipe", "inherit"] });
  await new Promise((ok, bad) => { srv.stdout.on("data", d => String(d).includes("ready") && ok()); srv.on("exit", c => bad(new Error(`server exited ${c}`))); });
  const url = `http://127.0.0.1:${port}/`;
  const b = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  p.on("pageerror", e => errors.push(e.message));
  p.on("console", m => { if (m.type() === "error") errors.push(m.text()); });
  await p.addInitScript(() => {
    window.__calls = [];
    window.__TAURI__ = { core: { invoke: async (cmd, args) => { window.__calls.push([cmd, args]); const r = await fetch("/invoke", { method: "POST", body: JSON.stringify({ cmd, args: args ?? {} }) }); const j = await r.json(); if ("err" in j) throw j.err; return j.ok; } }, event: { listen: async () => () => {} } };
  });
  const inv = (cmd, args) => p.evaluate(([c, a]) => window.__TAURI__.core.invoke(c, a), [cmd, args ?? {}]);
  const project = () => inv("state").then(o => o.p);
  const scr = (x, y) => p.evaluate(([x, y]) => window.__toScreen(x, y), [x, y]);
  const clickAt = async (x, y) => { const [sx, sy] = await scr(x, y); await p.mouse.click(sx, sy); await p.waitForTimeout(250); };
  const dragFrom = async (a, c) => { const [x0, y0] = await scr(...a), [x1, y1] = await scr(...c); await p.mouse.move(x0, y0); await p.mouse.down(); await p.mouse.move((x0 + x1) / 2, (y0 + y1) / 2, { steps: 4 }); await p.mouse.move(x1, y1, { steps: 4 }); await p.mouse.up(); await p.waitForTimeout(300); };
  const place = async (name, x, y) => { await p.fill(".psearch", name); await p.click(`.psym:has(span:text-is("${name}"))`); await clickAt(x, y); await p.fill(".psearch", ""); };
  const shot = async n => { if (shots) await p.screenshot({ path: `${shots}/${n}.png` }); };
  await p.goto(url);
  await p.waitForTimeout(900);

  // 1. File page: magenta sidebar, the update card, a blank project tile.
  check("File sidebar is the magenta accent", (await p.$eval(".bs aside", el => getComputedStyle(el).backgroundColor)) === "rgb(156, 31, 132)");
  check("the update card names Datum Schema", (await p.textContent(".card .brand")).includes("Datum Schema"));
  await shot("file");
  await p.click(".tile:has(b:text-is('Blank project'))"); await p.waitForTimeout(500);

  // 2. The window: ribbon tabs, the sheet with its frame and title block.
  check("ribbon tabs", JSON.stringify(await p.$$eval(".tabs > button:not(.file)", els => els.map(e => e.textContent))) === JSON.stringify(["Home", "Draw", "Symbols", "Lines", "Instruments", "Check", "Lists", "Sheets", "View", "Help"]));
  const frame = await p.$$eval(".frame text", els => els.map(e => e.textContent));
  check("the sheet has zones and a title block", ["A", "H", "8", "PID-001", "DRAWING NO.", "1 of 1"].every(t => frame.includes(t)), JSON.stringify(frame.slice(0, 12)));
  check("the palette has about 80 symbols", (await p.$$(".psym")).length >= 50);

  // 3. Place two vessels; they get tags V-101 and V-102.
  await place("Vertical vessel", 80, 100);
  await place("Vertical vessel", 260, 100);
  let pr = await project();
  check("placing a symbol from the palette", pr.items.length === 2 && pr.items[0].tag === "V-101" && pr.items[1].tag === "V-102", JSON.stringify(pr.items.map(i => i.tag)));

  // 4. Draw a line from the first vessel's right lower nozzle (90, 108) to the second's left lower (250, 108).
  await dragFrom([90, 108], [250, 108]);
  pr = await project();
  const l0 = pr.lines[0];
  check("dragging port to port draws a line", pr.lines.length === 1 && l0.a.at === "port" && l0.a.item === pr.items[0].id && l0.b.at === "port" && l0.b.item === pr.items[1].id, JSON.stringify(pr.lines));
  check("the new line gets the sheet's next sequence number", l0?.data.seq === "1001");

  // 5. Drop a gate valve on the line: it goes in, the line splits in two, both keep the data.
  await place("Gate valve", 170, 108.5);
  pr = await project();
  const gv = pr.items.find(i => i.sym === "gate");
  check("a valve dropped on a line splits it", pr.lines.length === 2 && !!gv && pr.lines.some(l => l.b.at === "port" && l.b.item === gv.id) && pr.lines.some(l => l.a.at === "port" && l.a.item === gv.id), JSON.stringify(pr.lines.map(l => [l.a, l.b])));
  check("the valve sits on the line", gv && Math.abs(gv.y - 108) < 1e-9);
  await shot("split");

  // 6. The inspector edits the title block, and the frame follows.
  await p.keyboard.press("Escape"); await p.click(".canvas", { position: { x: 30, y: 30 } }); await p.waitForTimeout(200);
  const company = p.locator(".insp .iprow:has(label:text-is('Company')) input");
  await company.fill("ACME Process <b>"); await company.press("Tab"); await p.waitForTimeout(400);
  const frame2 = await p.$$eval(".frame text", els => els.map(e => e.textContent));
  check("the title block shows the company as text", frame2.includes("ACME Process <b>"));

  // 7. Undo and redo.
  await p.click(".canvas", { position: { x: 30, y: 30 } }); await p.keyboard.press("Control+z"); await p.waitForTimeout(300);
  check("undo takes back the last edit", (await project()).title.company === "");
  await p.keyboard.press("Control+y"); await p.waitForTimeout(300);
  check("redo puts it back", (await project()).title.company === "ACME Process <b>");

  // 8. Save writes a .dschema that opens again with the same drawing.
  await p.keyboard.press("Control+s"); await p.waitForTimeout(300);
  const saved = (await inv("exports")).filter(e => e.kind === "dschema").pop();
  check("save writes the project", !!saved && JSON.parse(saved.text).items.length === 3);

  // 9. Dark theme draws without errors.
  await p.click(".theme"); await p.waitForTimeout(200);
  check("dark theme", (await p.evaluate(() => document.documentElement.dataset.theme)) === "dark");
  await shot("dark");

  check("no errors in the window", errors.length === 0, errors.join(" | "));
  await b.close();
  await fetch(`${url}quit`).catch(() => {});
  srv.kill();
  console.log(failed ? `${failed} UI checks failed` : "All UI checks passed");
  process.exit(failed ? 1 : 0);
})().catch(e => { console.error(e); process.exit(1); });
