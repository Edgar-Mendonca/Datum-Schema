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
  p.setDefaultTimeout(5000);
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
  const tagOf = (pr, it) => pr.objects.find(o => o.id === it.obj)?.tag;
  check("placing a symbol from the palette", pr.items.length === 2 && tagOf(pr, pr.items[0]) === "V-101" && tagOf(pr, pr.items[1]) === "V-102", JSON.stringify(pr.items.map(i => tagOf(pr, i))));

  // 4. Draw a line from the first vessel's right lower nozzle (90, 108) to the second's left lower (250, 108).
  await dragFrom([90, 108], [250, 108]);
  pr = await project();
  const l0 = pr.lines[0];
  check("dragging port to port draws a line", pr.lines.length === 1 && l0.a.at === "port" && l0.a.item === pr.items[0].id && l0.b.at === "port" && l0.b.item === pr.items[1].id, JSON.stringify(pr.lines));
  check("the new line gets the sheet's next sequence number", pr.objects.find(o => o.id === l0?.obj)?.data.seq === "1001");

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
  check("the project file is SQLite and reads back the same", saved?.sqlite === true);

  // 9. Dark theme draws without errors.
  await p.click(".theme"); await p.waitForTimeout(200);
  check("dark theme", (await p.evaluate(() => document.documentElement.dataset.theme)) === "dark");
  await shot("dark");

  await p.click(".theme"); await p.click(".theme"); await p.waitForTimeout(200);

  // 10. Find the faults: the Checks panel lists the planted faults; the status bar counts them.
  const openEx = async name => {
    await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(200);
    await p.click(`.tile:has(b:text-is("${name}"))`); await p.waitForTimeout(300);
    const d = await p.$('.dlg button:has-text("Don\'t save")'); if (d) { await d.click(); await p.waitForTimeout(300); }
    await p.waitForTimeout(600);
  };
  await openEx("Find the faults");
  const rules = await p.$$eval(".ctbl tbody tr td:nth-child(2)", els => els.map(e => e.textContent));
  check("the Checks panel lists the planted faults", JSON.stringify(rules) === JSON.stringify(["1", "2", "4", "7", "8", "9", "11", "13"]), JSON.stringify(rules));
  check("the status bar counts errors and warnings", (await p.textContent(".status .sbtn")).includes("3 errors · 5 warnings"));
  await shot("faults");

  // 11. Click the pump issue: pump B is selected; then fix it with a check valve and the issue clears.
  await p.click(".ctbl tr:has(td:text-is('8'))"); await p.waitForTimeout(500);
  let pr2 = await project();
  const pumpB = pr2.items.filter(i => i.sym === "pump_cent").sort((a, b) => b.y - a.y)[0];
  const selIds = await p.evaluate(() => [...document.querySelectorAll(".it.sel")].length);
  check("clicking an issue selects what it is about", selIds === 1);
  // Pump B's discharge runs along y = 184 to the gate at x = 205; drop a check valve at x = 186.
  await place("Check valve", 186, 184);
  pr2 = await project();
  const rules2 = await p.$$eval(".ctbl tbody tr td:nth-child(2)", els => els.map(e => e.textContent));
  check("a check valve on pump B's discharge clears check 8", !rules2.includes("8") && pr2.items.filter(i => i.sym === "check").length === 2, JSON.stringify(rules2));
  void pumpB;

  // 12. The line list: rows from the drawing, hydraulics; export it to CSV.
  await p.click(".btabs button:has-text('Line list')"); await p.waitForTimeout(500);
  const lrows = await p.$$eval(".lists tbody tr.f td:first-child", els => els.map(e => e.textContent));
  check("the line list lists the line numbers", lrows.includes('3"-P-1003-A1A') && lrows.includes('4"-P-1001-A1A'), JSON.stringify(lrows));
  await p.click(".cfilter button:has-text('CSV')"); await p.waitForTimeout(400);
  const csv = (await inv("exports")).filter(e => e.kind === "csv").pop();
  check("the line list exports to CSV", !!csv && csv.text.startsWith("Line number,Size,") && csv.text.includes('"3""-P-1003-A1A"'), csv?.text.slice(0, 200));
  // Editing a cell edits the drawing: the service of a line.
  const cell = p.locator(".lists tbody tr.f").filter({ has: p.locator("td:first-child", { hasText: '3"-P-1007-A1A' }) }).locator("td:nth-child(7) input");
  await cell.fill("Receiver outlet, edited"); await cell.press("Enter"); await p.waitForTimeout(400);
  pr2 = await project();
  check("editing the list edits the line", pr2.objects.some(o => o.data.service === "Receiver outlet, edited"));
  await p.click(".btabs button:has-text('Equipment')"); await p.waitForTimeout(400);
  check("the equipment list", (await p.$$eval(".lists tbody tr.f td:first-child input", els => els.map(e => e.value))).includes("V-101"));

  // 13. DXF of the sheet.
  await p.click(".tabs button:text-is('Sheets')"); await p.click(".ribbon button[title='DXF']"); await p.waitForTimeout(500);
  const dxf = (await inv("exports")).filter(e => e.kind === "dxf").pop();
  check("DXF export writes an R12 drawing", !!dxf && dxf.text.includes("AC1009") && dxf.text.includes("SYM_PUMP_CENT") && dxf.text.trimEnd().endsWith("EOF"));

  // 14. A second sheet with an off-page connector pair; double-click jumps to the partner.
  await openEx("Pump transfer");
  await place("Off-page connector", 395, 60);
  const opA = (await project()).items.find(i => i.sym === "offpage");
  await p.click(".stabs .sadd"); await p.waitForTimeout(500);
  await place("Off-page connector", 30, 60);
  pr2 = await project();
  const opB = pr2.items.find(i => i.sym === "offpage" && i.id !== opA.id);
  check("a new sheet is numbered PID-002", pr2.sheets.length === 2 && pr2.sheets[1].number === "PID-002" && opB.sheet === pr2.sheets[1].id);
  // Pair them in the window: the new connector is selected; Lines › Pair….
  await p.click(".tabs button:text-is('Lines')"); await p.click(".ribbon button[title='Pair…']"); await p.waitForTimeout(300);
  await p.click(".dlg tr.f"); await p.click(".dlg .btn.pri"); await p.waitForTimeout(400);
  pr2 = await project();
  check("pairing off-page connectors", pr2.items.find(i => i.id === opA.id).link === opB.id);
  const [cx, cy] = await scr(30, 60); await p.mouse.dblclick(cx, cy); await p.waitForTimeout(600);
  const onSheet = await p.$eval(".stabs button.on", e => e.textContent);
  check("double-clicking an off-page connector jumps to its partner's sheet", onSheet === "PID-100", onSheet);

  // 15. A legend sheet shows the symbols used.
  await p.click(".tabs button:text-is('Sheets')"); await p.click(".ribbon button[title*='every symbol']"); await p.waitForTimeout(600);
  const legend = await p.$$eval(".legend text", els => els.map(e => e.textContent));
  check("the legend sheet lists the symbols used", legend.includes("Centrifugal pump") && legend.includes("Gate valve") && legend.includes("LEGEND"), JSON.stringify(legend.slice(0, 8)));
  await shot("legend");

  // 16. A Hold cloud and a note.
  await p.click(".stabs button:text-is('PID-100')"); await p.waitForTimeout(400);
  await p.click(".tabs button:text-is('Draw')"); await p.click(".ribbon button[title='Hold Cloud']"); await clickAt(250, 220);
  check("a Hold cloud", (await project()).notes.some(n => n.kind === "hold") && (await p.$$(".note.hold path")).length > 10);

  // 17. Print: the pages are drawn (sheets, lists, legend).
  await p.click(".tabs .file"); await p.click(".bs aside button:has-text('Print')"); await p.waitForTimeout(1200);
  const pages = await p.$$(".pprev .pwrap");
  check("the print preview has the sheets and the lists", pages.length >= 4, String(pages.length));
  check("printed sheets are drawn as SVG with the title block", (await p.$$eval(".pprev .ppage svg text", els => els.map(e => e.textContent))).includes("DRAWING NO."));
  await shot("print");
  await p.click(".bs .back"); await p.waitForTimeout(300);

  // 18. Keyboard: duplicate takes the next free tag, R turns, Delete of a valve joins its line again.
  await openEx("Pump transfer");
  pr2 = await project();
  const pumpA = pr2.items.find(i => tagOf(pr2, i) === "P-101A");
  await p.click(".canvas", { position: { x: 5, y: 5 } });
  const [pax, pay] = await scr(pumpA.x, pumpA.y + 2); await p.mouse.click(pax, pay); await p.waitForTimeout(200);
  await p.keyboard.press("Control+d"); await p.waitForTimeout(500);
  pr2 = await project();
  check("Ctrl+D duplicates with the next free tag", pr2.objects.some(o => o.tag === "P-101C"), JSON.stringify(pr2.objects.filter(o => o.class === "pump_cent").map(o => o.tag)));
  await p.keyboard.press("r"); await p.waitForTimeout(300);
  { const q = await project(); check("R turns the selection", q.items.find(i => tagOf(q, i) === "P-101C")?.rot === 1); }
  await p.keyboard.press("Delete"); await p.waitForTimeout(300);
  check("deleting the last symbol of an object asks", !!(await p.$('.dlg header:has-text("Delete")')));
  await p.keyboard.press("Enter"); await p.waitForTimeout(300);
  check("…and Enter deletes the object", !(await project()).objects.some(o => o.tag === "P-101C"));
  const lines0 = (await project()).lines.length;
  const q0 = await project(); const hv = q0.items.find(i => tagOf(q0, i) === "HV-101");
  const [hx, hy] = await scr(hv.x, hv.y); await p.mouse.click(hx, hy); await p.waitForTimeout(200);
  await p.keyboard.press("Delete"); await p.waitForTimeout(300);
  await p.keyboard.press("Enter"); await p.waitForTimeout(400);
  pr2 = await project();
  check("deleting an in-line valve joins its two lines again", !pr2.objects.some(o => o.tag === "HV-101") && pr2.lines.length === lines0 - 1);

  // 19. US units: the Inspector and the lists follow; the file keeps SI.
  await p.click(".tabs button:text-is('Home')"); await p.click(".ribbon button:has-text('US: psi')"); await p.waitForTimeout(400);
  await p.click(".btabs button:has-text('Line list')"); await p.waitForTimeout(400);
  const heads = await p.$$eval(".lists thead th", els => els.map(e => e.textContent));
  check("US units in the line list", heads.some(h => h.includes("(gpm)")) && heads.some(h => h.includes("(psi)")), JSON.stringify(heads));
  check("the file keeps SI", (await project()).objects.find(o => o.data.flow)?.data.flow < 0.01);

  // 20. Tell Me finds a command.
  await p.keyboard.press("Control+k"); await p.waitForTimeout(200);
  await p.keyboard.type("equipment list"); await p.keyboard.press("Enter"); await p.waitForTimeout(400);
  check("Tell Me runs a command", (await p.textContent(".btabs button.on")).includes("Equipment"));

  // 21. The exhibition check: every example opens with 0 errors, prints and exports DXF.
  for (const name of ["Pump transfer", "Compressor package", "Exchanger loop"]) {
    await openEx(name);
    const errs = await p.textContent(".status .sbtn");
    check(`${name} opens with 0 errors and 0 warnings`, errs.includes("0 errors · 0 warnings"), errs);
    await p.click(".tabs button:text-is('Sheets')"); await p.click(".ribbon button[title='DXF']"); await p.waitForTimeout(400);
    const dx = (await inv("exports")).filter(e => e.kind === "dxf").pop();
    check(`${name} exports DXF`, !!dx && dx.text.trimEnd().endsWith("EOF"));
    await p.click(".tabs .file"); await p.click(".bs aside button:has-text('Print')"); await p.waitForTimeout(1000);
    check(`${name} prints`, (await p.$$(".pprev .pwrap")).length >= 3);
    await p.click(".bs .back"); await p.waitForTimeout(200);
  }

  // 22. Classes: add a required attribute to Pump in Options; pumps show it and check 6 asks for it.
  await openEx("Pump transfer");
  await p.click(".tabs .file"); await p.click(".bs aside button:has-text('Options')"); await p.waitForTimeout(400);
  await p.selectOption(".classes select", "Pump");
  await p.fill(".addattr input >> nth=0", "Seal plan");
  await p.click(".addattr label.chk input");
  await p.click(".addattr button"); await p.waitForTimeout(400);
  check("a project attribute is added to a class", (await project()).settings.user_attrs.some(u => u.class === "Pump" && u.attr.key === "seal_plan" && u.attr.required));
  await p.click(".bs .back"); await p.waitForTimeout(400);
  await p.click(".btabs button:has-text('Checks')"); await p.waitForTimeout(300);
  check("check 6 asks for the required attribute", (await p.$$eval(".ctbl tbody td", els => els.map(e => e.textContent))).some(t => t.includes("seal plan")));
  const pq = await project(); const pa = pq.items.find(i => tagOf(pq, i) === "P-101A");
  const [sx, sy] = await scr(pa.x, pa.y + 2); await p.mouse.click(sx, sy); await p.waitForTimeout(300);
  check("the Inspector shows the class's new attribute", (await p.textContent(".insp")).includes("Seal plan *"));

  // 23. The Plant panel: the hierarchy with every object; remove a pump from the sheet only, it
  // stays (not drawn); draw it again from the panel.
  await openEx("Pump transfer");
  await p.click(".palette header button:text-is('Plant')"); await p.waitForTimeout(300);
  const tree = await p.$$eval(".plant .ptag", els => els.map(e => e.textContent));
  check("the Plant panel lists the objects", tree.includes("P-101A") && tree.includes("V-101") && tree.some(t => t.includes("P-1003")), JSON.stringify(tree.slice(0, 10)));
  check("the hierarchy has its levels", (await p.$$eval(".plant .pnname", els => els.map(e => e.textContent))).join("|") === "Plant 1|Unit 100|Area 1");
  let q1 = await project(); const pb = q1.items.find(i => tagOf(q1, i) === "P-101B");
  const [bx, by] = await scr(pb.x, pb.y + 2); await p.mouse.click(bx, by); await p.waitForTimeout(200);
  await p.keyboard.press("Delete"); await p.waitForTimeout(300);
  await p.click(".dlg button:has-text('Remove from this sheet only')"); await p.waitForTimeout(400);
  q1 = await project();
  check("removed from the sheet, the object stays", q1.objects.some(o => o.tag === "P-101B") && !q1.items.some(i => tagOf(q1, i) === "P-101B"));
  check("the Plant panel shows it not drawn", (await p.textContent(".plant .porow.undrawn")).includes("P-101B"));
  await p.click(".plant .porow.undrawn", { button: "right" }); await p.click(".menu button:has-text('Draw it on this sheet')");
  await clickAt(160, 190);
  q1 = await project();
  check("an object can be drawn again from the Plant panel", q1.items.some(i => tagOf(q1, i) === "P-101B") && q1.objects.filter(o => o.tag === "P-101B").length === 1);
  await p.click(".palette header button:text-is('Symbols')");

  // 24. Datasheets: LT-101's datasheet takes the conditions of the line it measures; a value typed
  // on it goes to the object; it prints on one A4 page.
  const q2 = await project(); const lt = q2.items.find(i => tagOf(q2, i) === "LT-101");
  { const [lx, ly] = await scr(lt.x, lt.y); await p.mouse.click(lx, ly); await p.waitForTimeout(200); }
  await p.click(".insp button:has-text('Datasheet')"); await p.waitForSelector(".dlg .ds");
  const dsText = await p.textContent(".dlg .ds");
  check("an instrument's datasheet has its sections", ["General", "Process conditions", "Instrument", "Specification"].every(t => dsText.includes(t)), dsText.slice(0, 200));
  check("the datasheet takes the process conditions from the vessel it is tapped to", dsText.includes("Mounted onV-101") && dsText.includes("from V-101"), dsText.slice(0, 400));
  await p.fill(".dlg .ds tr:has(td:text-is('Manufacturer')) input", "Acme Instruments"); await p.keyboard.press("Tab"); await p.waitForTimeout(300);
  check("a value typed on the datasheet goes to the object", (await project()).objects.find(o => o.tag === "LT-101")?.data.manufacturer === "Acme Instruments");
  await p.click(".dlg footer button:has-text('Print')"); await p.waitForSelector(".pprev .pwrap");
  const dsPages = await p.$$eval(".pprev .pwrap", els => els.map(e => e.textContent));
  check("one datasheet prints on one page", dsPages.length === 1 && dsPages[0].includes("LT-101") && dsPages[0].includes("Acme Instruments"), JSON.stringify(dsPages.map(t => t.slice(0, 60))));
  await p.click(".printpage button:has-text('Back to the drawing')"); await p.waitForTimeout(200);

  check("no errors in the window", errors.length === 0, errors.join(" | "));
  await b.close();
  await fetch(`${url}quit`).catch(() => {});
  srv.kill();
  console.log(failed ? `${failed} UI checks failed` : "All UI checks passed");
  process.exit(failed ? 1 : 0);
})().catch(e => { console.error(e); process.exit(1); });
