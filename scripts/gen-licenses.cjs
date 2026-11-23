const fs=require('fs');
const m=JSON.parse(fs.readFileSync('C:/vektora-tools/meta.json','utf8'));
const pk=m.packages.filter(p=>!p.name.startsWith("trasea")).map(p=>[p.name,p.version,p.license||'see crate']).sort((a,b)=>a[0].localeCompare(b[0]));
let md='# Third-party licenses\n\nTrasea is MIT licensed. It bundles or links the following. Potrace (GPL) is NOT used.\n\n## Bundled programs and models\n\n| Component | License | Source |\n|---|---|---|\n| Real-ESRGAN (model realesrgan-x4plus-anime) | BSD-3-Clause (check the model-file terms again before selling the app) | https://github.com/xinntao/Real-ESRGAN |\n| Real-ESRGAN-ncnn-vulkan (realesrgan-ncnn-vulkan.exe, release v0.2.5.0) | MIT | https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan |\n| ncnn (inside that executable) | BSD-3-Clause | https://github.com/Tencent/ncnn |\n| vcomp140.dll (OpenMP runtime) | Microsoft Visual C++ redistributable file, redistributable under Microsoft license terms | https://learn.microsoft.com/cpp/windows/redistributing-visual-cpp-files |\n| VTracer / visioncortex (tracing engine) | MIT | https://github.com/visioncortex/vtracer |\n| React, React DOM | MIT | https://react.dev |\n\n## Rust crates linked into the app ('+pk.length+')\n\n| Crate | Version | License |\n|---|---|---|\n';
for(const p of pk) md+='| '+p[0]+' | '+p[1]+' | '+String(p[2]).replace(/\|/g,'/')+' |\n';
fs.writeFileSync('THIRD_PARTY_LICENSES.md',md);
console.log('crates',pk.length);
console.log('GPL-only:',pk.filter(p=>/GPL/.test(p[2])&&!/(MIT|Apache|BSD)/.test(p[2])).map(p=>p.join(' ')));
