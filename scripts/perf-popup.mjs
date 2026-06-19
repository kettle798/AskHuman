#!/usr/bin/env node
// Popup-launch performance harness (spec docs/specs/popup-launch-performance.md §7).
//
// Drives N non-interactive AskHuman invocations with perf instrumentation enabled, stitches the
// per-invocation milestones from ~/.askhuman/perf.log into a timeline, aggregates per-segment
// stats (median / p90), prints a table, and optionally compares against a saved baseline —
// exiting non-zero when the end-to-end p90 regresses beyond a threshold (default +20%).
//
// Each popup auto-cancels right after first paint (ASKHUMAN_PERF_AUTODISMISS=1), so no human is
// needed. Run `./scripts/install.sh` first so the on-disk binary carries the instrumentation.
//
// Isolation: the harness runs its own daemon under a throwaway HOME (a temp dir), so it never
// touches the user's real ~/.askhuman daemon or any in-flight asks. All paths (daemon.sock, lock,
// perf.log, agents.json, ...) live under $HOME/.askhuman, so redirecting HOME for the spawned
// CLI/daemon/helper fully isolates them. This also makes --cold (stop the daemon before each run)
// safe, since only the isolated daemon is stopped.
//
// Usage:
//   node scripts/perf-popup.mjs [options]
//     --runs N            iterations (default 20)
//     --bin PATH          AskHuman binary (default: $ASKHUMAN_BIN | ~/.local/bin/AskHuman | PATH)
//     --baseline FILE     compare current p90 against this baseline JSON
//     --save-baseline F   write current aggregate to F (as the new baseline)
//     --threshold P       regression threshold in percent on e2e p90 (default 20)
//     --timeout MS        per-run timeout before killing the invocation (default 30000)
//     --cold              stop the isolated daemon before each run (measure daemon cold start)
//     --json FILE         also dump the full aggregate JSON to FILE
//     --warmup N          discard the first N runs from aggregation (default 1)
//     --keep-home         keep the temp HOME dir after exit (for debugging)
//     -h, --help          show this help

import { spawn, spawnSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync, mkdtempSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";

// ---- arg parsing -----------------------------------------------------------

function parseArgs(argv) {
  const o = {
    runs: 20,
    bin: null,
    baseline: null,
    saveBaseline: null,
    threshold: 20,
    timeout: 30000,
    cold: false,
    json: null,
    warmup: 1,
    keepHome: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const next = () => argv[++i];
    switch (a) {
      case "--runs": o.runs = parseInt(next(), 10); break;
      case "--bin": o.bin = next(); break;
      case "--baseline": o.baseline = next(); break;
      case "--save-baseline": o.saveBaseline = next(); break;
      case "--threshold": o.threshold = parseFloat(next()); break;
      case "--timeout": o.timeout = parseInt(next(), 10); break;
      case "--cold": o.cold = true; break;
      case "--json": o.json = next(); break;
      case "--warmup": o.warmup = parseInt(next(), 10); break;
      case "--keep-home": o.keepHome = true; break;
      case "-h": case "--help": printHelp(); process.exit(0); break;
      default:
        console.error(`unknown option: ${a}`);
        printHelp();
        process.exit(2);
    }
  }
  return o;
}

function printHelp() {
  const text = readFileSync(new URL(import.meta.url)).toString();
  const lines = text.split("\n");
  for (const l of lines) {
    if (l.startsWith("// ")) console.log(l.slice(3));
    else if (l === "//") console.log("");
    else if (l.startsWith("#!")) continue;
    else break;
  }
}

// ---- binary resolution -----------------------------------------------------

function resolveBin(explicit) {
  const candidates = [];
  if (explicit) candidates.push(explicit);
  if (process.env.ASKHUMAN_BIN) candidates.push(process.env.ASKHUMAN_BIN);
  candidates.push(join(homedir(), ".local", "bin", "AskHuman"));
  for (const c of candidates) {
    if (c && existsSync(c)) return c;
  }
  // fall back to PATH lookup
  const which = spawnSync("which", ["AskHuman"], { encoding: "utf8" });
  if (which.status === 0) {
    const p = which.stdout.trim();
    if (p) return p;
  }
  // last resort: repo release build
  const repo = join(new URL("..", import.meta.url).pathname, "src-tauri", "target", "release", "AskHuman");
  if (existsSync(repo)) return repo;
  console.error("could not locate the AskHuman binary; pass --bin PATH or run scripts/install.sh");
  process.exit(2);
}

// ---- run helpers -----------------------------------------------------------

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

/** Run a daemon control subcommand against the isolated HOME (blocking). */
function daemonCmd(bin, home, args) {
  return spawnSync(bin, ["daemon", ...args], {
    stdio: "ignore",
    env: { ...process.env, HOME: home },
  });
}

/** Spawn one AskHuman ask against the isolated HOME; resolves when the process exits. */
function runOnce(bin, home, timeoutMs) {
  return new Promise((resolve) => {
    // Stamp the spawn instant and inject it so the CLI records it under this run's perf_id;
    // this yields a true end-to-end (spawn->painted) that includes OS process creation + load.
    const spawnTs = Date.now();
    const child = spawn(
      bin,
      ["AskHuman perf probe", "-q", "perf probe (auto-dismiss)", "-o", "ok", "-o", "cancel"],
      {
        stdio: "ignore",
        env: {
          ...process.env,
          HOME: home,
          ASKHUMAN_PERF: "1",
          ASKHUMAN_PERF_AUTODISMISS: "1",
          ASKHUMAN_PERF_SPAWN_TS: String(spawnTs),
        },
      },
    );
    let done = false;
    const finish = (how) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      resolve(how);
    };
    const timer = setTimeout(() => {
      try { child.kill("SIGKILL"); } catch { /* ignore */ }
      finish("timeout");
    }, timeoutMs);
    child.on("exit", () => finish("exit"));
    child.on("error", () => finish("error"));
  });
}

// ---- perf.log parsing ------------------------------------------------------

// Canonical milestone order (for reference / table grouping).
const STAGES = [
  "spawn", "cli.start", "cli.detect_done", "cli.submit",
  "dmn.submit_recv", "dmn.created", "dmn.accepted", "dmn.im_done", "dmn.spawned",
  "gui.start", "gui.show_recv", "gui.build_start", "gui.win_show", "gui.build_done",
  "fe.bootstrap", "fe.mounted", "fe.popup_init_done", "fe.painted",
];

// Named segments: [label, fromStage, toStage]. Indented labels are sub-segments of the line above.
// NOTE: gui.build_done marks when Tauri's `build()` returns (builder config); the heavy native
// window creation + first page load happen afterwards during run()/setup, so the meaningful
// "window visible" / "page boot" numbers are measured from gui.build_start / gui.show_recv.
const METRICS = [
  ["e2e+spawn (spawn->painted)", "spawn", "fe.painted"],
  ["  proc spawn (->cli.start)", "spawn", "cli.start"],
  ["e2e (cli.start->fe.painted)", "cli.start", "fe.painted"],
  ["cli (start->submit)", "cli.start", "cli.submit"],
  ["  detect", "cli.start", "cli.detect_done"],
  ["ipc (submit->dmn.recv)", "cli.submit", "dmn.submit_recv"],
  ["daemon (recv->spawned)", "dmn.submit_recv", "dmn.spawned"],
  ["  im_attach", "dmn.accepted", "dmn.im_done"],
  ["spawn->gui proc start", "dmn.spawned", "gui.start"],
  ["gui connect (start->show)", "gui.start", "gui.show_recv"],
  ["GUI total (show->painted)", "gui.show_recv", "fe.painted"],
  ["  tauri build()", "gui.build_start", "gui.build_done"],
  ["  window visible", "gui.build_start", "gui.win_show"],
  ["  page boot (->fe boot)", "gui.show_recv", "fe.bootstrap"],
  ["  frontend (boot->painted)", "fe.bootstrap", "fe.painted"],
  ["    popup_init", "fe.mounted", "fe.popup_init_done"],
];

/** Parse perf.log lines with ts >= floor into { perfId -> { stage -> minTs } }. */
function parsePerfLog(path, floorMs) {
  if (!existsSync(path)) return {};
  const groups = {};
  for (const line of readFileSync(path, "utf8").split("\n")) {
    if (!line) continue;
    const [tsStr, perfId, stage] = line.split("\t");
    const ts = Number(tsStr);
    if (!perfId || !stage || !Number.isFinite(ts) || ts < floorMs) continue;
    const g = (groups[perfId] ||= {});
    // Keep the earliest timestamp per stage (cli.submit can repeat on retry).
    if (g[stage] === undefined || ts < g[stage]) g[stage] = ts;
  }
  return groups;
}

// ---- stats -----------------------------------------------------------------

function percentile(sorted, p) {
  if (sorted.length === 0) return null;
  if (sorted.length === 1) return sorted[0];
  const rank = (p / 100) * (sorted.length - 1);
  const lo = Math.floor(rank);
  const hi = Math.ceil(rank);
  if (lo === hi) return sorted[lo];
  return sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo);
}

function summarize(values) {
  const v = values.filter((x) => Number.isFinite(x)).sort((a, b) => a - b);
  if (v.length === 0) return { count: 0, min: null, median: null, p90: null, max: null };
  return {
    count: v.length,
    min: v[0],
    median: percentile(v, 50),
    p90: percentile(v, 90),
    max: v[v.length - 1],
  };
}

function aggregate(groups) {
  // Only complete invocations (have both ends of the e2e segment) count.
  const complete = Object.values(groups).filter(
    (g) => g["cli.start"] !== undefined && g["fe.painted"] !== undefined,
  );
  const metrics = {};
  for (const [label, from, to] of METRICS) {
    const vals = [];
    for (const g of complete) {
      if (g[from] !== undefined && g[to] !== undefined) vals.push(g[to] - g[from]);
    }
    metrics[label] = summarize(vals);
  }
  return { complete: complete.length, total: Object.keys(groups).length, metrics };
}

// ---- reporting -------------------------------------------------------------

function fmt(n) {
  if (n === null || n === undefined) return "  -";
  return n.toFixed(1).padStart(7);
}

function printTable(agg, baseline, threshold) {
  const baseMetrics = baseline?.metrics ?? null;
  console.log("");
  console.log(
    `runs: ${agg.complete} complete / ${agg.total} total` +
      (baseline ? `   (baseline: ${baseline.complete ?? "?"} runs)` : ""),
  );
  console.log("");
  const head =
    "segment".padEnd(32) +
    "count".padStart(6) +
    "min".padStart(8) +
    "median".padStart(8) +
    "p90".padStart(8) +
    "max".padStart(8) +
    (baseMetrics ? "  base p90".padStart(10) + "   delta" : "");
  console.log(head);
  console.log("-".repeat(head.length));
  for (const [label] of METRICS) {
    const m = agg.metrics[label];
    let row =
      label.padEnd(32) +
      String(m.count).padStart(6) +
      fmt(m.min).padStart(8) +
      fmt(m.median).padStart(8) +
      fmt(m.p90).padStart(8) +
      fmt(m.max).padStart(8);
    if (baseMetrics) {
      const b = baseMetrics[label];
      if (b && b.p90 != null && b.p90 > 0 && m.p90 != null) {
        const deltaPct = ((m.p90 - b.p90) / b.p90) * 100;
        const sign = deltaPct >= 0 ? "+" : "";
        const flag = deltaPct > threshold ? " !" : "";
        row += fmt(b.p90).padStart(10) + `  ${sign}${deltaPct.toFixed(1)}%${flag}`;
      } else {
        row += fmt(b?.p90).padStart(10) + "   -";
      }
    }
    console.log(row);
  }
  console.log("");
}

// ---- main ------------------------------------------------------------------

async function main() {
  const o = parseArgs(process.argv.slice(2));
  const bin = resolveBin(o.bin);

  // Isolated throwaway HOME → its own daemon/socket/perf.log; never touches the real daemon.
  const home = mkdtempSync(join(tmpdir(), "askhuman-perf-"));
  const perfLog = join(home, ".askhuman", "perf.log");
  console.log(`AskHuman: ${bin}`);
  console.log(`isolated HOME: ${home}`);
  console.log(`perf.log: ${perfLog}`);

  let exitCode = 0;
  try {
    if (!o.cold) {
      // Warm: bring the isolated daemon up once and keep it across runs.
      console.log("starting isolated daemon...");
      daemonCmd(bin, home, ["start"]);
      await sleep(500);
    }

    // Everything appended at/after this instant belongs to this harness run.
    const floor = Date.now();
    await sleep(5);

    console.log(`running ${o.runs} iterations${o.cold ? " (cold: stop daemon each run)" : ""}...`);
    let timeouts = 0;
    for (let i = 0; i < o.runs; i++) {
      if (o.cold) {
        // Stop the isolated daemon so this run pays a full daemon cold start.
        daemonCmd(bin, home, ["stop", "--force"]);
        await sleep(200);
      }
      const how = await runOnce(bin, home, o.timeout);
      if (how !== "exit") timeouts++;
      process.stdout.write(`\r  ${i + 1}/${o.runs} (${how})    `);
      await sleep(150);
    }
    process.stdout.write("\n");
    if (timeouts > 0) console.warn(`warning: ${timeouts} run(s) did not exit cleanly`);

    // Give the last frontend marks (flushed via async IPC) a moment to hit disk.
    await sleep(400);

    exitCode = report(o, perfLog, floor);
  } finally {
    // Always tear down the isolated daemon and temp HOME.
    daemonCmd(bin, home, ["stop", "--force"]);
    if (!o.keepHome) {
      try { rmSync(home, { recursive: true, force: true }); } catch { /* ignore */ }
    } else {
      console.log(`kept isolated HOME: ${home}`);
    }
  }
  process.exit(exitCode);
}

/** Parse, aggregate, print and (optionally) gate against baseline. Returns the process exit code. */
function report(o, perfLog, floor) {
  let groups = parsePerfLog(perfLog, floor);
  // Drop warmup invocations (earliest by cli.start) to avoid cold-start skew.
  if (o.warmup > 0) {
    const ordered = Object.entries(groups)
      .filter(([, g]) => g["cli.start"] !== undefined)
      .sort((a, b) => a[1]["cli.start"] - b[1]["cli.start"]);
    for (const [id] of ordered.slice(0, o.warmup)) delete groups[id];
  }

  const agg = aggregate(groups);
  const baseline = o.baseline && existsSync(o.baseline)
    ? JSON.parse(readFileSync(o.baseline, "utf8"))
    : null;

  printTable(agg, baseline, o.threshold);

  const out = {
    generatedAt: new Date().toISOString(),
    runs: o.runs,
    complete: agg.complete,
    metrics: agg.metrics,
  };
  if (o.json) {
    writeFileSync(o.json, JSON.stringify(out, null, 2));
    console.log(`wrote aggregate JSON: ${o.json}`);
  }
  if (o.saveBaseline) {
    writeFileSync(o.saveBaseline, JSON.stringify(out, null, 2));
    console.log(`wrote baseline: ${o.saveBaseline}`);
  }

  if (agg.complete === 0) {
    console.error("error: no complete invocations were captured (is the installed binary instrumented?)");
    return 1;
  }

  // Regression gate: end-to-end p90 vs baseline.
  if (baseline) {
    // Prefer the spawn-inclusive e2e when it has data; else fall back to the cli.start-based one.
    const spawnLabel = "e2e+spawn (spawn->painted)";
    const cliLabel = "e2e (cli.start->fe.painted)";
    const e2eLabel = agg.metrics[spawnLabel]?.count > 0 ? spawnLabel : cliLabel;
    const cur = agg.metrics[e2eLabel]?.p90;
    const base = baseline.metrics?.[e2eLabel]?.p90;
    if (cur != null && base != null && base > 0) {
      const deltaPct = ((cur - base) / base) * 100;
      if (deltaPct > o.threshold) {
        console.error(
          `REGRESSION: e2e p90 ${cur.toFixed(1)}ms vs baseline ${base.toFixed(1)}ms ` +
            `(+${deltaPct.toFixed(1)}% > ${o.threshold}% threshold)`,
        );
        return 1;
      }
      console.log(
        `OK: e2e p90 ${cur.toFixed(1)}ms vs baseline ${base.toFixed(1)}ms ` +
          `(${deltaPct >= 0 ? "+" : ""}${deltaPct.toFixed(1)}%, within ${o.threshold}%)`,
      );
    }
  }
  return 0;
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
