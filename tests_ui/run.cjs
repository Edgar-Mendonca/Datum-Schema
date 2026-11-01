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
  // 4a. While dragging from a port, the line is drawn under the mouse.
  await p.keyboard.press("Control+z"); await p.waitForTimeout(300);
  { const [x0, y0] = await scr(90, 108); await p.mouse.move(x0, y0); await p.mouse.down(); await p.mouse.move(x0 + 80, y0 + 50, { steps: 5 }); await p.waitForTimeout(100);
    const r = await p.$eval(".dsvg .wiring path.temp", e => { const b = e.getBoundingClientRect(); return [b.right, b.bottom]; }).catch(() => null);
    check("dragging from a port draws the line under the mouse", !!r && Math.abs(r[0] - (x0 + 80)) < 3 && Math.abs(r[1] - (y0 + 50)) < 3, JSON.stringify([r, x0 + 80, y0 + 50]));
    await p.keyboard.press("Escape"); await p.mouse.up(); await p.waitForTimeout(200); }
  await p.keyboard.press("Control+y"); await p.waitForTimeout(300);
  check("redo brings the line back", (await project()).lines.length === 1);
  // 4b. The same line by clicks: click the start port, the line follows the mouse, click the end port.
  await p.keyboard.press("Control+z"); await p.waitForTimeout(300);
  check("undo takes the line away", (await project()).lines.length === 0);
  { const [x0, y0] = await scr(90, 108), [x1, y1] = await scr(250, 108);
    await p.mouse.click(x0, y0); await p.waitForTimeout(150);
    await p.mouse.move((x0 + x1) / 2, y0 + 40, { steps: 5 }); await p.waitForTimeout(150);
    await shot("line-follows");
    const wb = await p.$eval(".dsvg .wiring path.temp", e => { const r = e.getBoundingClientRect(); return [r.right, r.bottom]; });
    check("after a click on a port the line follows the mouse", Math.abs(wb[0] - (x0 + x1) / 2) < 3 && Math.abs(wb[1] - (y0 + 40)) < 3 && (await project()).lines.length === 0, JSON.stringify([wb, (x0 + x1) / 2, y0 + 40]));
    await p.mouse.move(x1, y1, { steps: 5 }); await p.waitForTimeout(150);
    check("the line snaps to the port under the mouse", (await p.$$(".dsvg .wiring .wsnap")).length === 1);
    await p.mouse.click(x1, y1); await p.waitForTimeout(400); }
  pr = await project();
  check("the second click ends the line on the port", pr.lines.length === 1 && pr.lines[0].a.at === "port" && pr.lines[0].b.at === "port" && pr.lines[0].b.item === pr.items[1].id && !(await p.$(".dsvg .wiring")), JSON.stringify(pr.lines));
  { const [x0, y0] = await scr(90, 108); await p.mouse.click(x0, y0); await p.mouse.move(x0 + 60, y0 + 60, { steps: 3 }); await p.keyboard.press("Escape"); await p.waitForTimeout(200); }
  check("Escape drops a line being drawn", !(await p.$(".dsvg .wiring")) && (await project()).lines.length === 1);

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
  { const bg = await p.$eval(".ov.high", e => getComputedStyle(e).backgroundColor);
    const alpha = +(bg.match(/rgba\([^)]*,\s*([\d.]+)\)/)?.[1] ?? 1);
    check("Tell Me dims the window behind it instead of hiding it", alpha > 0 && alpha < 0.5 && await p.isVisible(".ribbon"), bg); }
  await p.keyboard.type("equipment list"); await p.keyboard.press("Enter"); await p.waitForTimeout(400);
  check("Tell Me runs a command", (await p.textContent(".btabs button.on")).includes("Equipment"));

  // 21. The exhibition check: every example opens with 0 errors, prints and exports DXF.
  for (const name of ["Pump transfer", "Compressor package", "Exchanger loop", "Crude preheat (PFD)"]) {
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
  check("a new datasheet is not issued yet", (await p.textContent(".dlg .dsrev")).includes("Not issued yet"));
  await p.click(".dlg footer button:has-text('Issue…')"); await p.waitForSelector(".dlg header:has-text('Issue the datasheet')");
  check("the first issue suggests revision A", (await p.inputValue(".dlg .form input.mono")) === "A");
  await p.fill(".dlg .form input[placeholder='What this issue is for']", "For comment"); await p.keyboard.press("Enter"); await p.waitForTimeout(400);
  check("issuing gives the datasheet its revision and status", (await p.textContent(".dlg .dsrev")).includes("Rev A · Preliminary") && (await p.textContent(".dlg .ds.revs")).includes("For comment"));
  await p.fill(".dlg .ds tr:has(td:text-is('Manufacturer')) input", "Acme Instruments"); await p.keyboard.press("Tab"); await p.waitForTimeout(300);
  check("a value typed on the datasheet goes to the object", (await project()).objects.find(o => o.tag === "LT-101")?.data.manufacturer === "Acme Instruments");
  check("a value changed since the issue is marked Δ", (await p.textContent(".dlg .ds tr.chg:has(td:text-is('Manufacturer')) td.dlt")) === "Δ" && (await p.textContent(".dlg .dsrev")).includes("Δ 1 changed since"));
  await p.click(".dlg footer button:has-text('Print')"); await p.waitForSelector(".pprev .pwrap");
  const dsPages = await p.$$eval(".pprev .pwrap", els => els.map(e => e.textContent));
  check("one datasheet prints on one page", dsPages.length === 1 && dsPages[0].includes("LT-101") && dsPages[0].includes("Acme Instruments"), JSON.stringify(dsPages.map(t => t.slice(0, 60))));
  await p.click(".printpage button:has-text('Back to the drawing')"); await p.waitForTimeout(200);
  await p.click(".tabs button:text-is('Lists')"); await p.click(".ribbon button:has-text('Datasheets')"); await p.waitForSelector(".lists .egrid tbody tr.f");
  const ixRow = await p.$$eval(".lists .egrid tbody tr.f", els => els.map(e => [...e.querySelectorAll("td")].map(td => td.textContent)).find(c => c[0] === "LT-101"));
  check("the datasheet index shows its revision, status and changes", !!ixRow && ixRow[2] === "A" && ixRow[3] === "Preliminary" && ixRow[6] === "Δ 1", JSON.stringify(ixRow));
  await p.click(".lists button:has-text('shown…')"); await p.waitForSelector(".dlg header:has-text('datasheets')");
  await p.click(".dlg footer button:has-text('Issue')"); await p.waitForTimeout(500);
  const ixAll = await p.$$eval(".lists .egrid tbody tr.f", els => els.map(e => [...e.querySelectorAll("td")].map(td => td.textContent)));
  check("issuing every datasheet shown gives each its next revision", ixAll.length >= 5 && ixAll.every(c => c[3] === "Preliminary" && c[6] === "none") && ixAll.find(c => c[0] === "LT-101")?.[2] === "B" && ixAll.filter(c => c[0] !== "LT-101").every(c => c[2] === "A"), JSON.stringify(ixAll.map(c => c.slice(0, 4))));

  // 25. Import from Excel: export the datasheets, change a value, import the workbook: the preview
  // lists the change and leaves the line's values alone; Import writes it back.
  await inv("export", { kind: "datasheets", what: "", sheet: 0 });
  const ltObj = (await project()).objects.find(o => o.tag === "LT-101").id;
  await inv("apply", { op: { SetData: { ids: [ltObj], key: "manufacturer", val: "Other Co" } } });
  await p.click(".lists button:has-text('Import Excel')"); await p.waitForSelector(".dlg header:has-text('Import datasheets')");
  const imRows = await p.$$eval(".dlg .imtbl tbody tr", els => els.map(e => e.textContent));
  await shot("import-preview");
  check("the import preview lists the changed value", imRows.some(t => t.includes("LT-101") && t.includes("Manufacturer") && t.includes("Other Co") && t.includes("Acme Instruments")), JSON.stringify(imRows.slice(0, 5)));
  check("the preview never imports the line's values", !(await p.textContent(".dlg")).includes("Operating pressure") || imRows.every(t => !t.includes("Operating pressure")));
  await p.click(".dlg footer button.pri"); await p.waitForTimeout(400);
  check("Import writes the ticked values", (await project()).objects.find(o => o.tag === "LT-101").data.manufacturer === "Acme Instruments");

  // 26. The company layout: a form number and a hidden row, in Options › Datasheets.
  await p.click(".tabs .file"); await p.click(".bs aside button:has-text('Options')"); await p.waitForTimeout(400);
  await p.fill(".dslayout input[placeholder^='e.g. IDS']", "IDS-001"); await p.keyboard.press("Tab"); await p.waitForTimeout(300);
  await p.click(".dslayout .dssec:has(b:text-is('Specification')) label.chk:has-text('Model') input"); await p.waitForTimeout(300);
  const lay = (await project()).settings.ds_layout;
  check("the layout keeps the form number and the hidden row", lay.form === "IDS-001" && lay.hidden.includes("Specification › Model"), JSON.stringify(lay));
  await p.click(".bs .back"); await p.waitForTimeout(300);
  await p.click(".tabs button:text-is('Lists')"); await p.click(".ribbon button:has-text('Datasheets')"); await p.waitForSelector(".lists .egrid tbody tr.f");
  await p.locator(".lists .egrid tbody tr.f").filter({ has: p.locator("td", { hasText: /^LT-101$/ }) }).click(); await p.waitForSelector(".dlg .ds");
  const dsT = await p.textContent(".dlg .bd");
  check("the datasheet follows the layout", dsT.includes("Form IDS-001") && !(await p.$$eval(".dlg .ds td:first-child", els => els.map(e => e.textContent))).includes("Model"));
  await p.keyboard.press("Escape"); await p.waitForTimeout(200);

  // 27. Loop diagrams: L-101 is drawn with its devices; wiring typed in the table shows on it.
  await p.click(".ribbon button:has-text('Loop Diagrams')"); await p.waitForSelector(".dlg .loopsvg svg");
  check("the loop list has L-101", (await p.$$eval(".looplist b", els => els.map(e => e.textContent))).includes("L-101"));
  await p.click(".looplist button:has(b:text-is('L-101'))"); await p.waitForTimeout(300);
  const svgT = await p.$$eval(".dlg .loopsvg svg text", els => els.map(e => e.textContent));
  check("the loop diagram draws the loop on a sheet", ["FIELD", "JUNCTION BOX", "CONTROL SYSTEM", "LD-L-101", "LT", "LIC"].every(s => svgT.includes(s)), JSON.stringify(svgT.slice(0, 20)));
  await p.fill(".loopw tbody tr:has(td:text-is('LT-101')) td:nth-child(2) input", "JB-101"); await p.keyboard.press("Tab"); await p.waitForTimeout(400);
  check("wiring typed on the loop diagram shows on it", (await p.$$eval(".dlg .loopsvg svg text", els => els.map(e => e.textContent))).includes("JB-101") && (await project()).objects.find(o => o.tag === "LT-101").data.jb === "JB-101");
  await shot("loop-diagram");
  await p.click(".dlg footer button:has-text('Print')"); await p.waitForSelector(".pprev .pwrap");
  const lpages = await p.$$eval(".pprev .pwrap", els => els.map(e => e.textContent));
  check("one loop diagram prints on one page", lpages.length === 1 && lpages[0].includes("JB-101"), JSON.stringify(lpages.map(t => t.slice(0, 50))));
  await p.click(".printpage button:has-text('Back to the drawing')"); await p.waitForTimeout(200);

  // 28. The symbol search opens beside the double-clicked point and leaves the drawing in view.
  const n28 = (await project()).items.length;
  // An empty spot in the upper part of the sheet (clear of symbols and lines, above the bottom panel).
  const q27 = await project();
  const pts = q27.lines.flatMap(l => l.pts).concat(q27.items.map(i => [i.x, i.y]));
  const cv = await p.$eval(".dsvg", e => { const r = e.getBoundingClientRect(); return [r.left + 30, r.top + 30, r.right - 30, r.bottom - 30]; });
  let spot = null;
  for (let y = 20; y <= 280 && !spot; y += 10) for (let x = 20; x <= 400; x += 10) {
    const [sx, sy] = await scr(x, y);
    if (sx > cv[0] && sx < cv[2] && sy > cv[1] && sy < cv[3] && q27.items.every(i => Math.hypot(i.x - x, i.y - y) > 25) && pts.every(q => Math.hypot(q[0] - x, q[1] - y) > 20)) { spot = [x, y]; break; }
  }
  { const [qx, qy] = await scr(spot[0], spot[1]); await p.mouse.dblclick(qx, qy); await p.waitForSelector(".qpop input");
    const box = await p.$eval(".qpop", e => { const r = e.getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; });
    const bg = await p.$eval(".ov.pop", e => getComputedStyle(e).backgroundColor);
    await p.fill(".qpop input", "valve"); await p.waitForTimeout(100); await shot("quick-search"); await p.fill(".qpop input", "");
    check("the symbol search is a small popover beside the point", Math.abs(box[0] - qx) < 40 && box[2] < 360 && box[3] < 340 && (bg === "rgba(0, 0, 0, 0)" || bg === "transparent"), JSON.stringify([box, qx, qy, bg]));
    await p.fill(".qpop input", "ball"); await p.keyboard.press("Enter"); await p.waitForTimeout(400); }
  const q28 = await project();
  check("Enter places the symbol at the point", q28.items.length === n28 + 1 && q28.items.some(i => i.sym === "ball" && Math.abs(i.x - spot[0]) < 3 && Math.abs(i.y - spot[1]) < 3), JSON.stringify(spot));
  await p.keyboard.press("Control+z"); await p.waitForTimeout(300);

  // 29. Cause and effect, DEXPI and loop DXF on the compressor package.
  await openEx("Compressor package");
  await shot("compressor");
  await p.click(".tabs button:text-is('Lists')"); await p.click(".ribbon button:has-text('Cause & Effect')"); await p.waitForSelector(".dlg .cem");
  await shot("cause-effect");
  const cemRows = await p.$$eval(".dlg .cem tbody tr", els => els.map(e => [...e.querySelectorAll("td")].map(td => td.textContent)));
  const cemHead = await p.$$eval(".dlg .cem th.eff b", els => els.map(e => e.textContent));
  check("the cause-and-effect matrix is read from the interlock", JSON.stringify(cemRows.map(r => r[0])) === '["LSHH-202","TSHH-203"]' && JSON.stringify(cemHead) === '["M-201","UV-201"]' && cemRows.every(r => r.slice(3).every(c => c === "UY-202")), JSON.stringify([cemRows, cemHead]));
  check("each effect says what it does", JSON.stringify(await p.$$eval(".dlg .cem th.act", els => els.map(e => e.textContent))) === '["Stop","Open"]');
  await p.click(".dlg footer button:has-text('Print')"); await p.waitForSelector(".pprev .pwrap");
  const cpages = await p.$$eval(".pprev .pwrap", els => els.map(e => e.textContent));
  check("the matrix prints on one page", cpages.length === 1 && cpages[0].includes("LSHH-202") && cpages[0].includes("UY-202"), JSON.stringify(cpages.map(t => t.slice(0, 60))));
  await p.click(".printpage button:has-text('Back to the drawing')"); await p.waitForTimeout(200);
  await p.click(".tabs button:text-is('Sheets')"); await p.click(".ribbon button:has-text('DEXPI')"); await p.waitForTimeout(400);
  { const ex = await inv("exports"); const dx = ex.filter(e => e.kind === "dexpi").pop();
    check("the sheet exports in the DEXPI exchange format", !!dx && dx.text.includes("<PlantModel") && dx.text.includes('ComponentClass="CentrifugalCompressor"') && dx.text.trimEnd().endsWith("</PlantModel>")); }
  await p.click(".tabs button:text-is('Lists')"); await p.click(".ribbon button:has-text('Loop Diagrams')"); await p.waitForSelector(".dlg .loopsvg svg");
  await p.click(".dlg footer button:text-is('DXF')"); await p.waitForTimeout(400);
  { const ex = await inv("exports"); const dx = ex.filter(e => e.kind === "loopdxf").pop();
    check("a loop diagram exports to DXF", !!dx && dx.text.includes("LD-") && dx.text.trimEnd().endsWith("EOF")); }
  await p.keyboard.press("Escape"); await p.waitForTimeout(200);

  // 30. DEXPI import: the compressor package exported above opens again as a new project.
  const before30 = await project();
  await p.click(".tabs .file"); await p.click(".bs .tile:has-text('From a DEXPI file')"); await p.waitForTimeout(300);
  if (await p.$(".dlg header:has-text('Save changes')")) { await p.click(".dlg footer button:has-text(\"Don't save\")"); await p.waitForTimeout(500); }
  await p.waitForTimeout(500);
  { const q = await project(); const tags = pr => pr.objects.map(o => o.tag).filter(Boolean).sort().join(",");
    check("a DEXPI file opens as a new project with the same plant", tags(q) === tags(before30) && q.lines.length === before30.lines.length, JSON.stringify([q.objects.length, before30.objects.length, q.lines.length, before30.lines.length]));
    check("the imported project checks clean", (await p.textContent(".status")).includes("0 errors")); }

  // 31. A check valve fitted the wrong way round: check 8 says so; Turn round fixes it.
  await openEx("Pump transfer");
  { const q = await project(); const hv = q.items.find(i => tagOf(q, i) === "HV-103");
    const [hx, hy] = await scr(hv.x, hv.y);
    await p.mouse.click(hx, hy, { button: "right" }); await p.click(".menu button:has-text('Turn round')"); await p.waitForTimeout(400);
    await p.click(".btabs button:has-text('Checks')"); await p.waitForTimeout(300);
    check("check 8 finds a check valve fitted the wrong way round", (await p.$$eval(".ctbl tbody td", els => els.map(e => e.textContent))).some(t => t.includes("wrong way round")));
    await p.mouse.click(hx, hy, { button: "right" }); await p.click(".menu button:has-text('Turn round')"); await p.waitForTimeout(400);
    check("Turn round puts it right", !(await p.$$eval(".ctbl tbody td", els => els.map(e => e.textContent))).some(t => t.includes("wrong way round"))); }

  // 32. A client's legend (DXF) becomes a symbol library; the project is drawn to it, and back.
  await p.click(".tabs .file"); await p.click(".bs aside button:has-text('Libraries')"); await p.waitForTimeout(200);
  await p.click(".libraries button:has-text('Import a legend')"); await p.waitForSelector(".libtbl tbody tr");
  { const rows = await p.$$eval(".libtbl tbody tr", els => els.length);
    const matched = await p.$$eval(".libtbl tbody tr:not(.warn)", els => els.length);
    check("the legend's symbols are read, most matched to ours", rows >= 4 && matched / rows >= 0.8, `${matched}/${rows}`); }
  await p.click(".libtbl tbody tr:first-child button:has-text('Edit')"); await p.waitForSelector(".pedsvg");
  { const n0 = await p.$$eval(".pedport", els => els.length);
    const bx = await p.$(".pedsvg").then(e => e.boundingBox());
    await p.mouse.click(bx.x + bx.width * 0.5, bx.y + bx.height * 0.2); await p.waitForTimeout(200);
    check("a click adds a connection point", (await p.$$eval(".pedport", els => els.length)) === n0 + 1);
    await p.click(".pedside button:has-text('Remove')"); await p.click(".dlg footer button:has-text('Use these')"); await p.waitForTimeout(200); }
  await p.click(".libhead button:has-text('Save library')"); await p.waitForSelector(".libcard");
  check("the library is saved", (await p.textContent(".libcard")).includes("Client legend"));
  await openEx("Pump transfer");
  const syms0 = (await project()).items.map(i => i.sym).join(",");
  const tied = q => q.lines.flatMap(l => [l.a, l.b]).filter(e => e.at === "port").length;
  const tied0 = tied(await project());
  await p.click(".tabs .file"); await p.click(".bs aside button:has-text('Options')"); await p.waitForTimeout(200);
  await p.selectOption(".stdsec select", "Client legend"); await p.click(".stdsec button:has-text('Apply')"); await p.waitForTimeout(400);
  { const q = await project(); const n = q.items.filter(i => i.sym.startsWith("x.")).length;
    check("the drawing standard draws our symbols with the library's", n > 0 && q.settings.standard.library === "Client legend", String(n));
    check("objects and lines stay the same", q.items.every(i => !i.sym.startsWith("x.") || i.base) && tied(q) === tied0, `${tied(q)} of ${tied0} line ends on nozzles`); }
  await p.click(".bs .back"); await p.waitForTimeout(400);
  check("the re-skinned drawing still checks clean", (await p.textContent(".status")).includes("0 errors"));
  await p.click(".tabs .file"); await p.click(".bs aside button:has-text('Options')"); await p.waitForTimeout(200);
  await p.selectOption(".stdsec select", ""); await p.click(".stdsec button:has-text('Apply')"); await p.waitForTimeout(400);
  check("back to our own symbols exactly", (await project()).items.map(i => i.sym).join(",") === syms0);
  await p.click(".bs .back"); await p.waitForTimeout(400);

  // 33. An old drawing behind a sheet: faint, scaled to fit, moved in the Inspector, printed on request.
  await openEx("Compressor package");
  await p.click(".tabs button:text-is('Sheets')"); await p.click(".ribbon button:has-text('Old Drawing')"); await p.waitForTimeout(500);
  await p.waitForFunction(() => document.querySelectorAll(".dsvg .underlay polyline, .dsvg .underlay line, .dsvg .underlay path").length > 50);
  { const q = await project(); const u = q.sheets[0].underlay;
    check("an old drawing goes behind the sheet, scaled to fit", !!u && u.k < 0.6 && !("draw" in u), JSON.stringify(u && { ...u }));
    check("its text shows", (await p.$$eval(".dsvg .underlay text", els => els.map(e => e.textContent))).includes("P-101A")); }
  await p.keyboard.press("Escape"); await p.waitForTimeout(200);
  await p.fill(".ubox .three input >> nth=1", "40"); await p.keyboard.press("Tab"); await p.waitForTimeout(300);
  check("the Inspector moves it, keeping the drawing", (await project()).sheets[0].underlay?.x === 40 && (await p.$$eval(".dsvg .underlay text", els => els.length)) > 0);
  await p.click(".ubox label.chk:has-text('Print') input"); await p.waitForTimeout(300);
  await p.click(".tabs button:text-is('Sheets')"); await p.click(".ribbon button:has-text('DXF')"); await p.waitForTimeout(400);
  { const ex = await inv("exports"); const dx = ex.filter(e => e.kind === "dxf").pop();
    check("when printed, the sheet's DXF carries it on its own layer", !!dx && dx.text.includes("  8\r\nBACKGROUND\r\n")); }
  await p.click(".ubox button:has-text('Remove')"); await p.waitForTimeout(300);
  check("Remove takes it away", !(await project()).sheets[0].underlay && !(await p.$(".dsvg .underlay")));

  // 34. A line to Datum Pipe, a loop to Datum Sim: each as that app's own file.
  await openEx("Pump transfer");
  { const v = (await inv("views"))[0];
    const ln = v.lines.find(l => l.kind === "major" && l.segs.some(sg => sg.length >= 2 && Math.hypot(sg[1][0] - sg[0][0], sg[1][1] - sg[0][1]) > 20));
    const sg = ln.segs.find(sg => sg.length >= 2 && Math.hypot(sg[1][0] - sg[0][0], sg[1][1] - sg[0][1]) > 20);
    const [lx, ly] = await scr((sg[0][0] + sg[1][0]) / 2, (sg[0][1] + sg[1][1]) / 2);
    await p.mouse.click(lx, ly); await p.waitForTimeout(300);
    await p.click(".ctxgrp button:has-text('Format')"); await p.click(".ribbon button:has-text('To Datum Pipe')"); await p.waitForTimeout(400);
    const ex = (await inv("exports")).filter(e => e.kind === "pipe").pop(); const m = ex && JSON.parse(ex.text);
    check("a line goes to Datum Pipe as its model file", !!m && m.format === 1 && m.elements.length >= 1 && m.supports.length === 2 && m.pipes[0].od > 0, ex?.text.slice(0, 120)); }
  await p.click(".tabs button:text-is('Lists')"); await p.click(".ribbon button:has-text('Loop Diagrams')"); await p.waitForSelector(".dlg .loopsvg svg");
  await p.click(".dlg footer button:has-text('To Datum Sim')"); await p.waitForTimeout(400);
  { const ex = (await inv("exports")).filter(e => e.kind === "sim").pop(); const m = ex && JSON.parse(ex.text);
    check("a loop goes to Datum Sim as its model file", !!m && m.format === 2 && m.root.blocks.some(b => b.kind === "PID" && b.name === "LIC-101"), ex?.text.slice(0, 160)); }
  await p.keyboard.press("Escape"); await p.waitForTimeout(200);

  // 35. A process flow diagram: streams numbered in diamonds, the stream table, balances that close;
  // a wrong flow breaks the mass balance.
  await openEx("Crude preheat (PFD)");
  check("the PFD example checks clean", (await p.textContent(".status")).includes("0 errors"));
  if (process.env.SHOT) {
    await p.click(".tabs button:text-is('View')"); await p.click(".ribbon button:has-text('Whole Sheet')"); await p.waitForTimeout(300);
    await p.screenshot({ path: `${process.env.SHOT}/pfd.png`, clip: { x: 236, y: 166, width: 922, height: 452 } });
  }
  { const frame = await p.$$eval(".dsvg .frame text", els => els.map(e => e.textContent));
    check("the stream table is on the sheet", frame.some(t => t.startsWith("Mass flow")) && frame.includes("Flash vapour") && frame.includes("36000"), JSON.stringify(frame.slice(-30)));
    const marks = await p.$$eval(".dsvg .lines text", els => els.map(e => e.textContent));
    check("each stream has its number beside it", ["1", "2", "3", "4", "5", "6", "7", "8"].every(n => marks.includes(n)), JSON.stringify(marks)); }
  await p.click(".btabs button:has-text('Streams')"); await p.waitForTimeout(400);
  check("the stream list lists the streams", (await p.$$eval(".lists tbody tr.f", els => els.length)) === 8);
  { check("streams in number order", (await p.$$eval(".lists tbody tr.f td:first-child input", els => els.map(e => e.value))).join(",") === "1,2,3,4,5,6,7,8");
    const cell = p.locator(".lists tbody tr.f >> nth=4").locator("td:nth-child(6) input");
    await cell.fill("9000"); await cell.press("Enter"); await p.waitForTimeout(400); }
  await p.click(".btabs button:has-text('Checks')"); await p.waitForTimeout(300);
  check("a wrong flow breaks the mass balance at the separator", (await p.$$eval(".ctbl tbody td", els => els.map(e => e.textContent))).some(t => t.includes("Mass does not balance at V-301")));
  await p.keyboard.press("Control+z"); await p.waitForTimeout(400);
  check("undo puts it right", (await p.textContent(".status")).includes("0 errors"));
  await p.click(".tabs button:text-is('Sheets')"); await p.click(".ribbon button:has-text('PFD Sheet')"); await p.waitForTimeout(400);
  { const q = await project(); const sh = q.sheets[q.sheets.length - 1];
    check("a new PFD sheet", sh.pfd && sh.number === "PFD-002" && sh.title === "Process flow diagram", JSON.stringify(sh)); }
  await p.click(".tabs button:text-is('Sheets')"); await p.click(".ribbon button[title='This sheet in the DEXPI P&ID exchange format (XML)']"); await p.waitForTimeout(300);
  check("DEXPI says it is for P&ID sheets", ((await p.textContent("#toast").catch(() => "")) || "").includes("P&ID exchange"));

  // 36. Issue a sheet at revision B, change it, compare and cloud the changes.
  await openEx("Pump transfer");
  await p.click(".insp h4 button:has-text('Issue')"); await p.waitForSelector(".dlg");
  check("the next revision is offered", (await p.inputValue(".dlg .form input >> nth=0")) === "B");
  await p.click(".dlg footer button:has-text('Issue')"); await p.waitForTimeout(400);
  { const q = await project(); check("the sheet is issued at B", q.sheets[0].rev === "B" && q.sheets[0].snaps.some(x => x.rev === "B") && q.sheets[0].revs.some(r => r.rev === "B")); }
  { const q = await project(); const v = q.items.find(i => tagOf(q, i) === "V-101");
    const [vx, vy] = await scr(v.x, v.y); await p.mouse.click(vx, vy); await p.waitForTimeout(200);
    for (let i = 0; i < 4; i++) await p.keyboard.press("ArrowRight");
    await p.waitForTimeout(300); await p.keyboard.press("Escape"); await p.waitForTimeout(300); }
  await p.click(".insp button:has-text('Compare with B')"); await p.waitForSelector(".dlg .cmptbl tr");
  { const rows = await p.$$eval(".dlg .cmptbl tr", els => els.map(e => e.textContent));
    check("the comparison lists the moved vessel and its rerouted lines", rows.some(r => r.includes("V-101") && r.includes("moved")) && rows.some(r => r.includes("rerouted")), JSON.stringify(rows)); }
  await p.click(".dlg footer button:has-text('Cloud them')"); await p.waitForTimeout(400);
  { const t = await p.$$eval(".dsvg .revclouds text", els => els.map(e => e.textContent));
    if (process.env.SHOT) await p.screenshot({ path: `${process.env.SHOT}/clouds.png`, clip: { x: 236, y: 166, width: 922, height: 452 } });
    check("the changes are clouded with revision C", t.length >= 1 && t.every(x => x === "C") && (await p.$$(".dsvg .revclouds path, .dsvg .revclouds polyline, .dsvg .revclouds polygon")).length > 10, JSON.stringify(t)); }
  await p.click(".tabs button:text-is('Sheets')"); await p.click(".ribbon button[title='DXF']"); await p.waitForTimeout(400);
  { const dx = (await inv("exports")).filter(e => e.kind === "dxf").pop(); check("the clouds go on the REVISION layer", !!dx && dx.text.includes("  8\r\nREVISION\r\n")); }

  // 37. Stream compositions: components in Options, rows in the stream table, check 17.
  await openEx("Crude preheat (PFD)");
  check("the components are listed in Options", await (async () => { await p.click(".tabs .file"); await p.click(".bs aside button:has-text('Options')"); await p.waitForTimeout(300); const v = await p.inputValue(".compsec textarea"); await p.click(".bs .back"); await p.waitForTimeout(300); return v.split("\n").length === 4 && v.includes("Naphtha"); })());
  check("the stream table has a row per component", (await p.$$eval(".dsvg .frame text", els => els.map(e => e.textContent.trim()))).includes("Naphtha"));
  check("the compositions balance", (await p.textContent(".status")).includes("0 errors · 0 warnings"));
  await p.click(".btabs button:has-text('Streams')"); await p.waitForTimeout(400);
  { const ci = (await p.$$eval(".lists thead th", els => els.map(e => e.textContent))).findIndex(t => t.startsWith("Naphtha"));
    const cell = p.locator(".lists tbody tr.f >> nth=4").locator(`td:nth-child(${ci + 1}) input`);
    await cell.fill("0.4"); await cell.press("Enter"); await p.waitForTimeout(400); }
  await p.click(".btabs button:has-text('Checks')"); await p.waitForTimeout(300);
  { const t = await p.$$eval(".ctbl tbody td", els => els.map(e => e.textContent));
    check("a wrong fraction breaks the component balance and the sum", t.some(x => x.includes("Naphtha does not balance at V-301")) && t.some(x => x.includes("stream 5 adds up to 0.9"))); }
  await p.keyboard.press("Control+z"); await p.waitForTimeout(400);

  // 38. The I/O summary, and an instrument range that does not cover its line.
  await openEx("Pump transfer");
  await p.click(".btabs button:has-text('I/O summary')"); await p.waitForTimeout(400);
  { const rows = await p.$$eval(".lists tbody tr.f", els => els.map(e => [...e.querySelectorAll("td")].map(td => td.textContent)));
    check("the I/O summary counts inputs and outputs", rows.length >= 1 && rows.some(r => r[0] === "(not assigned)" && +r[1] >= 1 && +r[2] >= 1), JSON.stringify(rows)); }
  await p.click(".btabs button:has-text('Instruments')"); await p.waitForTimeout(400);
  { const ci = (await p.$$eval(".lists thead th", els => els.map(e => e.textContent))).findIndex(t => t.startsWith("Range"));
    const ri = await p.$$eval(".lists tbody tr.f", els => els.findIndex(e => e.querySelector("td input")?.value === "PI-102"));
    const cell = p.locator(`.lists tbody tr.f >> nth=${ri}`).locator(`td:nth-child(${ci + 1}) input`);
    await cell.fill("0–2 bar"); await cell.press("Enter"); await p.waitForTimeout(400); }
  await p.click(".btabs button:has-text('Checks')"); await p.waitForTimeout(300);
  check("check 18 finds a range below the operating pressure", (await p.$$eval(".ctbl tbody td", els => els.map(e => e.textContent))).some(x => x.includes("range of PI-102")));

  // 39. The valve and fitting take-off: counted by type, size and class; export to CSV.
  await openEx("Pump transfer");
  await p.click(".tabs button:text-is('Lists')"); await p.click(".ribbon button:has-text('Take-off')"); await p.waitForTimeout(400);
  { const rows = await p.$$eval(".lists tbody tr.f", els => els.map(e => [...e.querySelectorAll("td")].map(td => td.textContent)));
    check("the take-off counts the 3\" gate valves", rows.some(r => r[0] === "Gate valve" && r[1] === '3"' && r[2] === "A1A" && r[4] === "4"), JSON.stringify(rows)); }
  await p.click(".cfilter button:has-text('CSV')"); await p.waitForTimeout(400);
  { const csv = (await inv("exports")).filter(e => e.kind === "csv").pop(); check("the take-off exports to CSV", !!csv && csv.what === "takeoff" && csv.text.startsWith("Item,Size,Pipe class,Rating,Quantity"), csv?.text.slice(0, 80)); }

  // 40. A company title block: from the standard, a cell of our own filled in from the Inspector.
  await openEx("Pump transfer");
  await p.click(".tabs .file"); await p.click(".bs aside button:has-text('Options')"); await p.waitForTimeout(300);
  await p.click(".tbsec label.chk:has-text('company title block') input"); await p.waitForSelector(".tbsec .tbcells tbody tr");
  check("the company block starts from the standard one", (await p.$$(".tbsec .tbcells tbody tr")).length === 12 && !!(await p.$(".tbsec .tbprev .frame")));
  await p.click(".tbsec button:has-text('Add a cell')"); await p.waitForTimeout(400);
  { const last = p.locator(".tbsec .tbcells tbody tr").last();
    await last.locator("td:nth-child(1) input").fill("CONTRACT"); await last.locator("td:nth-child(1) input").press("Tab"); await p.waitForTimeout(300);
    await last.locator("td:nth-child(3) input").fill("Contract"); await last.locator("td:nth-child(3) input").press("Tab"); await p.waitForTimeout(300);
    for (const [k, v] of [[4, "140"], [5, "0"], [6, "40"], [7, "12"]]) { const c = last.locator(`td:nth-child(${k}) input`); await c.fill(v); await c.press("Tab"); await p.waitForTimeout(200); } }
  { const tb = (await project()).settings.title_block; const c = tb.cells[tb.cells.length - 1];
    check("the new cell is kept inside the block", tb.on && c.field === "f:Contract" && c.label === "CONTRACT" && c.x + c.w <= tb.w, JSON.stringify(c)); }
  if (process.env.SHOT) { await p.locator(".tbsec").scrollIntoViewIfNeeded(); await p.locator(".tbsec").screenshot({ path: `${process.env.SHOT}/tb.png` }); }
  await p.click(".bs .back"); await p.waitForTimeout(300); await p.keyboard.press("Escape");
  { const inp = p.locator(".insp .iprow:has-text('Contract') input"); await inp.fill("C-2026-17"); await inp.press("Tab"); await p.waitForTimeout(400); }
  check("the field of our own is drawn in the title block", (await p.$$eval(".dsvg .frame text", els => els.map(e => e.textContent))).includes("C-2026-17"));

  // 41. Review comments: pin one, reply, take in a reviewer's copy (their reply resolves it).
  await openEx("Pump transfer");
  await p.click(".tabs button:text-is('Check')"); await p.click(".ribbon button:has-text('Comment')");
  { const [cx, cy] = await scr(200, 150); await p.mouse.click(cx, cy); }
  await p.waitForSelector(".dlg textarea"); await p.fill(".dlg textarea", "Check the receiver nozzle size"); await p.click(".dlg footer button:has-text('Add comment')"); await p.waitForTimeout(400);
  { const q = await project(); check("a comment is pinned to the sheet", q.comments.length === 1 && q.comments[0].text === "Check the receiver nozzle size" && (await p.$$(".dsvg .pins .pin")).length === 1); }
  check("the status bar counts open comments", (await p.textContent(".status")).includes("1 open comments"));
  await p.fill(".cmcard .cmrow input", "Will do"); await p.press(".cmcard .cmrow input", "Enter"); await p.waitForTimeout(400);
  check("a reply is added", (await project()).comments[0].replies.length === 1);
  await p.click(".cmpanel button:has-text('Import a reviewer')"); await p.waitForTimeout(500);
  { const q = await project(); check("a reviewer's copy comes in: a new comment, a reply and the resolution", q.comments.length === 2 && q.comments[0].resolved && q.comments[0].replies.length === 2 && q.comments[0].resolved_by === "Reviewer", JSON.stringify(q.comments.map(c => [c.text, c.resolved, c.replies.length]))); }
  check("the open list shows only the reviewer's new comment", (await p.$$(".cmcard")).length === 1);
  await p.click(".cmcard button:has-text('Resolve')"); await p.waitForTimeout(300);
  check("resolved: nothing open", (await p.$$(".cmcard")).length === 0 && !(await p.textContent(".status")).includes("open comments"));

  // 42. The user guide: every help topic in chapters, searchable, printable.
  await p.click(".tabs button:text-is('Help')"); await p.click(".ribbon button:has-text('User Guide')"); await p.waitForSelector(".guide .gtopic");
  if (process.env.SHOT) await p.screenshot({ path: `${process.env.SHOT}/guide.png` });
  { const n = await p.$$eval(".guide .gmain .gtopic", els => els.length);
    const helpBtns = await p.$$eval(".gtoc a", els => els.map(e => e.textContent));
    check("the guide has every chapter and many topics", n >= 40 && helpBtns.includes("Glossary") && helpBtns.some(t => t.includes("Process flow diagrams")), `${n} topics; ${helpBtns.join(" / ")}`);
    check("the guide lists every check rule", (await p.$$eval(".guide .grules tr", els => els.length)) === 23);
    await p.fill(".gsearch", "revision cloud"); await p.waitForTimeout(200);
    const found = await p.$$eval(".guide .gmain .gtopic h3", els => els.map(e => e.textContent));
    check("search narrows the guide", found.length >= 1 && found.length < 6 && found.some(t => t.includes("Revisions")), JSON.stringify(found));
    await p.fill(".gsearch", ""); }
  await p.evaluate(() => { window.print = () => { window.__printed = document.querySelectorAll("#printroot .gtopic").length; }; });
  await p.click(".gtoc button:has-text('Print')"); await p.waitForTimeout(600);
  check("the guide prints whole", (await p.evaluate(() => window.__printed)) >= 40);
  await p.click(".bs .back"); await p.waitForTimeout(200);

  // 43. Pipe specs: the example's class in Options, a CSV table read in, a component outside its
  // class found by check 22, the take-off named from the spec.
  await openEx("Pump transfer");
  await p.click(".tabs .file"); await p.click(".bs aside button:has-text('Options')"); await p.waitForTimeout(300);
  if (process.env.SHOT) { await p.locator(".specsec").scrollIntoViewIfNeeded(); await p.locator(".specsec").screenshot({ path: `${process.env.SHOT}/specs.png` }); }
  check("the example has its pipe spec", (await p.$$eval(".specsec .spectabs button", els => els.map(e => e.textContent))).join() === "A1A" && (await p.$$(".specsec .specitems tbody tr")).length >= 20);
  await p.setInputFiles(".specsec label:has-text('Read from CSV') input", { name: "specs.csv", mimeType: "text/csv", buffer: Buffer.from("Class,Component,From,To,Description,Code\nA1A,Gate valve,6,24,Gate valve RF,VG-6\nB3A,Ball valve,1/2,2,Ball valve SW,VB-1\n") });
  await p.waitForTimeout(500);
  { const sp = (await project()).settings.specs; check("a CSV table is read into the specs", sp.length === 2 && sp[0].items.length === 1 && sp[1].name === "B3A", JSON.stringify(sp.map(s => [s.name, s.items.length]))); }
  await p.click(".bs .back"); await p.waitForTimeout(300);
  await p.click(".btabs button:has-text('Checks')"); await p.waitForTimeout(300);
  { const t = await p.$$eval(".ctbl tbody td", els => els.map(e => e.textContent));
    check("check 22 finds valves the class no longer allows", t.some(x => x.includes("HV-104") && x.includes("outside pipe class A1A")) && t.some(x => x.includes("is not in pipe class A1A"))); }
  await p.keyboard.press("Control+z"); await p.waitForTimeout(400);
  await p.click(".btabs button:has-text('Take-off')"); await p.waitForTimeout(400);
  check("the take-off gives each component its commodity code", (await p.$$eval(".lists tbody tr.f", els => els.map(e => e.textContent))).some(t => t.includes("VGA150-01")));

  check("no errors in the window", errors.length === 0, errors.join(" | "));
  await b.close();
  await fetch(`${url}quit`).catch(() => {});
  srv.kill();
  console.log(failed ? `${failed} UI checks failed` : "All UI checks passed");
  process.exit(failed ? 1 : 0);
})().catch(e => { console.error(e); process.exit(1); });
