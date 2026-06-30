'use strict';

// One-off probe: sample the newest Claude transcript JSONL in a project dir to see
// whether the file grows incrementally during a long streaming response, or only
// jumps at message completion. Decides if a transcript-mtime "working" heartbeat is viable.
//
// Usage: node transcript-watch.cjs [projectDir] [seconds]
// Prints one line per sample whenever size/mtime changes (plus a heartbeat every ~5s).

const fs = require('fs');
const path = require('path');

const PROJ =
  process.argv[2] ||
  '/Users/wutian/.claude/projects/-Users-wutian-Developer-HumanInLoop-demo-agent-lifecycle-agents-claude';
const RUN_SECS = Number(process.argv[3] || 900);
const INTERVAL_MS = 500;

function newestJsonl(dir) {
  let best = null;
  try {
    for (const f of fs.readdirSync(dir)) {
      if (!f.endsWith('.jsonl')) continue;
      const p = path.join(dir, f);
      const st = fs.statSync(p);
      if (!best || st.mtimeMs > best.mtimeMs) best = { p, f, mtimeMs: st.mtimeMs, size: st.size };
    }
  } catch (e) {
    return null;
  }
  return best;
}

function hhmmss(ms) {
  return new Date(ms).toISOString().slice(11, 23);
}

const start = Date.now();
let lastKey = '';
let lastBeat = 0;
console.log(`watching ${PROJ} for ${RUN_SECS}s (interval ${INTERVAL_MS}ms)`);

const timer = setInterval(() => {
  const now = Date.now();
  if (now - start > RUN_SECS * 1000) {
    clearInterval(timer);
    console.log('done');
    return;
  }
  const cur = newestJsonl(PROJ);
  if (!cur) return;
  const key = `${cur.f}:${cur.size}:${Math.round(cur.mtimeMs)}`;
  if (key !== lastKey) {
    console.log(
      `${hhmmss(now)} CHANGED file=${cur.f} size=${cur.size} mtime=${hhmmss(cur.mtimeMs)} dSize=${
        cur.size - (Number(lastKey.split(':')[1]) || 0)
      }`
    );
    lastKey = key;
    lastBeat = now;
  } else if (now - lastBeat > 5000) {
    console.log(`${hhmmss(now)} idle    file=${cur.f} size=${cur.size} mtime=${hhmmss(cur.mtimeMs)}`);
    lastBeat = now;
  }
}, INTERVAL_MS);
