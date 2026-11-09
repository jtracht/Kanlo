import {readFileSync,writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const sha=execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim();
const rust=readFileSync('data/rust-tests.log','utf8');
const browser=readFileSync('data/browser-tests.log','utf8');
const debian=readFileSync('data/debian-validation.log','utf8');
const headless=readFileSync('data/headless-validation.log','utf8');
const sum=log=>[...log.matchAll(/test result: ok\. (\d+) passed;/g)].reduce((n,match)=>n+Number(match[1]),0);
if(/FAILED|test result: FAILED/.test(rust)||sum(rust)!==53||!/28 passed/.test(browser)||!/0 failed/.test(debian)||!headless.includes('PASS: Debian 13 headless'))throw new Error('Acceptance logs are incomplete or failed');
const architectures=[['x86_64-unknown-linux-gnu',62,2],['aarch64-unknown-linux-gnu',183,2],['armv7-unknown-linux-gnueabihf',40,1]].map(([target,machine,elfClass])=>{
  const path=`artifacts/kanlo-server-debug-${target}`;const data=readFileSync(path);const hash=createHash('sha256').update(data).digest('hex');
  if(!data.subarray(0,4).equals(Buffer.from([127,69,76,70]))||data[4]!==elfClass||data[5]!==1||data.readUInt16LE(18)!==machine||readFileSync(path+'.sha256','utf8').split(/\s+/)[0]!==hash)throw new Error(`Invalid architecture/checksum ${target}`);
  return {target,elf_machine:machine,elf_class:elfClass,sha256:hash,bytes:data.length};
});
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
const value={schema_version:1,source_commit:sha,environment:'Windows development + Debian 13 Docker validation; not physical Pi hardware',rust_tests:sum(rust),debian_rust_tests:sum(debian),browser_tests:28,browser_projects:['desktop 1440x1000','mobile 390x844 Chromium touch emulation'],https:'Isolated browser trusts only fixture SPKI; no certificate installed in user/system trust stores',headless:{debian:13,user:'kanlo',network:'none (loopback only)',capabilities:'all dropped',display:false,checks:['startup','restart','private directory permissions','health','SIGTERM with active SSE','SQLite integrity']},artifacts:architectures,asset_sha256:Object.fromEntries(['app.js','style.css','index.html','sw.js'].map(name=>[name,hash(`crates/kanlo-api/assets/${name}`)])),log_sha256:Object.fromEntries(['rust-tests','browser-tests','debian-validation','headless-validation'].map(name=>[name,hash(`data/${name}.log`)])),limitations:['No native TV runtime or physical Pi/SSD acceptance','No complete installer/image/updater or full OpenAPI/MCP/ChatGPT','No stable release or final license/trademark approval']};
writeFileSync('docs/validation/local-50.json',JSON.stringify(value,null,2)+'\n');
console.log(JSON.stringify({source_commit:sha,rust_tests:value.rust_tests,debian_rust_tests:value.debian_rust_tests,browser_tests:28,artifacts:architectures.length}));
