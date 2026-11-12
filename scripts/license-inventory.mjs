import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, existsSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';

// An inventory for owner review, not a compatibility or trademark clearance.
const metadata=JSON.parse(execFileSync('cargo',['metadata','--locked','--format-version','1'],{encoding:'utf8',maxBuffer:32*1024*1024}));
const rows=[];
for(const item of metadata.packages.filter(item=>!metadata.workspace_members.includes(item.id))){
  const directory=dirname(item.manifest_path);
  rows.push({ecosystem:'cargo',name:item.name,version:item.version,license:item.license??null,repository:item.repository??null,notices:collect(directory,`cargo/${item.name}-${item.version}`)});
}
for(const root of ['web','tests/browser','plugins/kanlo']){
  const lock=JSON.parse(readFileSync(`${root}/package-lock.json`,'utf8'));
  for(const [location,item] of Object.entries(lock.packages).filter(([location])=>location)){
    const name=item.name??location.slice(location.lastIndexOf('node_modules/')+13);
    rows.push({ecosystem:`npm:${root}`,name,version:item.version,license:item.license??null,optional:!!item.optional,notices:collect(join(root,location),`npm/${name.replaceAll('/','_')}-${item.version}`)});
  }
}
function collect(directory,target){
  if(!existsSync(directory))return [];
  const files=readdirSync(directory).filter(name=>/^(licen[sc]e|copying|copyright|notice)(\.|-|$)/i.test(name));
  return files.flatMap(name=>{try{const content=readFileSync(join(directory,name),'utf8');if(content.includes('\0'))return [];const destination=`docs/licenses/${target}/${name}`;mkdirSync(dirname(destination),{recursive:true});writeFileSync(destination,content);return [destination];}catch{return [];}});
}
rows.sort((a,b)=>`${a.ecosystem}:${a.name}:${a.version}`.localeCompare(`${b.ecosystem}:${b.name}:${b.version}`));
mkdirSync('docs/licenses',{recursive:true});
writeFileSync('docs/licenses/inventory.json',JSON.stringify({schema_version:1,legal_review:'pending',packages:rows},null,2)+'\n');
console.log(`Inventoried ${rows.length} locked package records; ${rows.filter(row=>!row.notices.length).length} require separate license-text collection.`);
