// Fails on high or critical npm advisories. An exception needs an entry in
// npm-audit-exceptions.json with a reason and an expiry date; expired
// exceptions fail again so they are reviewed.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const prefix = process.argv[2];
if (!prefix) {
  console.error("Usage: node scripts/npm-audit.mjs <package directory>");
  process.exit(2);
}
const exceptions = JSON.parse(
  readFileSync(fileURLToPath(new URL("../npm-audit-exceptions.json", import.meta.url)), "utf8"),
);
const today = new Date().toISOString().slice(0, 10);
let output;
try {
  output = execFileSync("npm", ["audit", "--json", "--prefix", prefix], {
    encoding: "utf8",
    shell: process.platform === "win32",
    maxBuffer: 64 * 1024 * 1024,
  });
} catch (error) {
  // npm audit exits non-zero when it finds vulnerabilities.
  output = error.stdout;
  if (!output) throw error;
}
const report = JSON.parse(output);
if (report.error) {
  console.error(`npm audit failed: ${report.error.summary ?? JSON.stringify(report.error)}`);
  process.exit(1);
}
const advisories = new Map();
for (const vulnerability of Object.values(report.vulnerabilities ?? {})) {
  for (const via of vulnerability.via) {
    if (typeof via === "object" && ["high", "critical"].includes(via.severity)) {
      advisories.set(via.url, via);
    }
  }
}
let failed = false;
for (const [url, advisory] of advisories) {
  const id = url.split("/").pop();
  const exception = exceptions[id];
  if (exception && exception.expires >= today && exception.packages.includes(advisory.name)) {
    console.log(`Accepted until ${exception.expires}: ${id} ${advisory.name} - ${exception.reason}`);
  } else {
    failed = true;
    const note = exception ? ` (exception expired ${exception.expires})` : "";
    console.error(`${advisory.severity}: ${id} ${advisory.name} ${advisory.title}${note}`);
  }
}
if (failed) process.exit(1);
console.log(`npm audit: no unaccepted high or critical advisories in ${prefix}`);
