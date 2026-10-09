/* IR Viewer Core: offline JavaScript; browser or Node; no dependencies. Independently implemented, not MAE source. */
(function (root) {
'use strict';
/*
MIT License
Copyright (c) 2026 IR Viewer contributors
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
of the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:
The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
*/
const IRCore = (() => {
 const MAX_BYTES=128*1024*1024, MAX_FRAMES=4194304;
 function parseWave(buffer){
  if(buffer.byteLength>MAX_BYTES)throw Error('文件超过 128 MiB 上限，请先裁定需要分析的 IR 文件。');
  if(buffer.byteLength<12)throw Error('文件太短，不是有效 WAV / IRS。');
  const v=new DataView(buffer), text=(o,n)=>String.fromCharCode(...new Uint8Array(buffer,o,n));
  const riff=text(0,4), rf64=riff==='RF64';
  if((riff!=='RIFF'&&!rf64)||text(8,4)!=='WAVE')throw Error('不支持这个文件：需要 RIFF WAV，或内部为 WAV 的 IRS。');
  let end=rf64?buffer.byteLength:v.getUint32(4,true)+8;
  if(end>buffer.byteLength||end<12)throw Error('WAV 文件被截断或 RIFF 长度无效。');
  let fmt=null, data=null, data64=null, saw64=false;
  const uint64=o=>{const n=v.getUint32(o,true)+v.getUint32(o+4,true)*4294967296;if(!Number.isSafeInteger(n))throw Error('RF64 长度超出安全整数范围。');return n;};
  for(let p=12;p<end;){
   if(p+8>end)throw Error('WAV 块头不完整。');
   const id=text(p,4);let size=v.getUint32(p+4,true);const start=p+8;
   if(size===0xffffffff){if(rf64&&id==='data'&&data64!==null)size=data64;else throw Error('不支持的 RF64 扩展块长度。');}
   if(size>end-start)throw Error('WAV 数据块被截断。');
   if(id==='ds64'&&rf64){
    if(size<28||saw64)throw Error('RF64 ds64 块无效。');
    saw64=true;data64=uint64(start+8);const declared=uint64(start)+8;
    if(declared>buffer.byteLength||declared<start+size)throw Error('RF64 文件长度无效。');end=declared;
   }
   if(id==='fmt '){
    if(fmt||size<16)throw Error('WAV 格式块缺失字段或重复。');
    let tag=v.getUint16(start,true),channels=v.getUint16(start+2,true),rate=v.getUint32(start+4,true),align=v.getUint16(start+12,true),bits=v.getUint16(start+14,true),valid=bits;
    if(tag===65534){
     if(size<40||v.getUint16(start+16,true)<22)throw Error('扩展 WAV 格式块不完整。');
     valid=v.getUint16(start+18,true)||bits;
     const tail=[0,0,16,0,128,0,0,170,0,56,155,113];
     if(!tail.every((b,i)=>v.getUint8(start+28+i)===b))throw Error('不支持的 WAV 子格式 GUID。');
     tag=v.getUint32(start+24,true);
    }
    if(!channels||channels>32||!rate||rate>768000)throw Error('不支持的声道数或采样率。');
    if(!((tag===1&&[8,16,24,32].includes(bits))||(tag===3&&[32,64].includes(bits))))throw Error('仅支持 PCM 8/16/24/32 位或浮点 32/64 位 WAV，不支持压缩 WAV。');
    if(valid>bits||valid<1||(tag===3&&valid!==bits))throw Error('WAV 有效位数无效。');
    if(align!==channels*(bits/8))throw Error('WAV 声道块对齐与位深不一致。');
    fmt={tag,channels,rate,align,bits,valid};
   }
   if(id==='data'){if(data)throw Error('暂不支持多个 data 块。');data={offset:start,bytes:size};}
   p=start+size+(size%2);
   if(p>end)throw Error('WAV 奇数字节块缺少填充。');
  }
  if(rf64&&!saw64)throw Error('RF64 缺少 ds64 块。');
  if(!fmt||!data)throw Error('WAV 缺少 fmt 或 data 块。');
  if(!data.bytes||data.bytes%fmt.align)throw Error('IR 数据为空或帧不完整。');
  const frames=data.bytes/fmt.align;if(frames>MAX_FRAMES)throw Error('IR 超过每声道 4,194,304 帧上限；未截短或分析部分文件。');
  return {...fmt,...data,frames,duration:frames/fmt.rate};
 }
 function decodeChannel(buffer,meta,channel){
  if(!Number.isInteger(channel)||channel<0||channel>=meta.channels)throw Error('声道编号无效。');
  const v=new DataView(buffer),a=new Float64Array(meta.frames),b=meta.bits/8;
  for(let i=0,p=meta.offset+channel*b;i<a.length;i++,p+=meta.align){
   let s;
   if(meta.tag===3)s=meta.bits===32?v.getFloat32(p,true):v.getFloat64(p,true);
   else if(meta.bits===8)s=(v.getUint8(p)-128)/128;
   else if(meta.bits===16)s=v.getInt16(p,true)/32768;
   else if(meta.bits===24){let q=v.getUint8(p)|(v.getUint8(p+1)<<8)|(v.getUint8(p+2)<<16);if(q&0x800000)q-=0x1000000;s=q/8388608;}
   else s=v.getInt32(p,true)/2147483648;
   if(!Number.isFinite(s))throw Error('IR 含 NaN 或 Infinity，无法计算。');a[i]=s;
  }return a;
 }
 function spectrum(samples){
  let n=65536;while(n<samples.length)n*=2;
  const re=new Float64Array(n),im=new Float64Array(n);re.set(samples);
  for(let i=1,j=0;i<n;i++){let bit=n>>1;for(;j&bit;bit>>=1)j^=bit;j^=bit;if(i<j){const t=re[i];re[i]=re[j];re[j]=t;}}
  for(let len=2;len<=n;len*=2){const angle=-2*Math.PI/len,wr0=Math.cos(angle),wi0=Math.sin(angle),half=len/2;
   for(let i=0;i<n;i+=len){let wr=1,wi=0;for(let j=0;j<half;j++){const a=i+j,b=a+half,tr=re[b]*wr-im[b]*wi,ti=re[b]*wi+im[b]*wr;re[b]=re[a]-tr;im[b]=im[a]-ti;re[a]+=tr;im[a]+=ti;const next=wr*wr0-wi*wi0;wi=wr*wi0+wi*wr0;wr=next;}}
  }
  const power=new Float64Array(n/2+1);for(let i=0;i<power.length;i++){power[i]=re[i]*re[i]+im[i]*im[i];if(!Number.isFinite(power[i]))throw Error('IR 幅度过大，FFT 数值溢出。');}return {power,n};
 }
 const toDb=p=>p>0 ? 10*Math.log10(p) : -Infinity;
 function viewCurve(power,rate,n,smoothing,points=1200,minHz=20,maxHz=20000){
  const fmax=Math.min(maxHz,rate/2),fmin=minHz,df=rate/n,ratio=fmax/fmin,xy=[];
  // Fractional-octave smoothing: average power using a rolling frequency band.
  let left=0,right=-1,sum=0;
  function smoothed(f){const factor=2**(1/(2*smoothing)),lo=Math.max(0,Math.ceil(f/factor/df)),hi=Math.min(power.length-1,Math.floor(f*factor/df));
   if(hi<lo)return power[Math.min(power.length-1,Math.round(f/df))];
   while(right<hi)sum+=power[++right];while(left<lo)sum-=power[left++];
   return Math.max(0,sum/(hi-lo+1));
  }
  for(let i=0;i<points;i++){
   const f=fmin*ratio**(i/(points-1));
   if(smoothing){xy.push([f,toDb(smoothed(f))]);continue;}
   const fl=i===0?fmin:fmin*ratio**((i-.5)/(points-1)),fh=i===points-1?fmax:fmin*ratio**((i+.5)/(points-1));
   const lo=Math.max(0,Math.ceil(fl/df)),hi=Math.min(power.length-1,Math.floor(fh/df));
   if(hi>=lo){let imin=lo,imax=lo;for(let k=lo+1;k<=hi;k++){if(power[k]<power[imin])imin=k;if(power[k]>power[imax])imax=k;}const indices=imin===imax?[imin]:[imin,imax].sort((a,b)=>a-b);for(const k of indices)xy.push([Math.max(fmin,k*df),toDb(power[k])]);}
   else{const bin=f/df,k=Math.floor(bin),t=bin-k;xy.push([f,toDb(power[k]*(1-t)+(power[k+1]??power[k])*t)]);}
  }return {xy,fmin,fmax,df};
 }
 return {parseWave,decodeChannel,spectrum,viewCurve,toDb,MAX_BYTES};
})();


/** Load once; reuse FFT results when display options change.
 * Synchronous API: use a Worker for large files in an interactive application.
 */
function loadIR(buffer) {
    if (!(buffer instanceof ArrayBuffer)) throw new TypeError('Expected ArrayBuffer');
    const meta = IRCore.parseWave(buffer);
    const cache = new Map();
    function spectrumFor(channel) {
        if (!cache.has(channel)) {
            const value = IRCore.spectrum(IRCore.decodeChannel(buffer, meta, channel));
            if (cache.size >= 4) cache.delete(cache.keys().next().value);
            cache.set(channel, value);
        }
        return cache.get(channel);
    }
    function response({
        channel = 0, smoothing = 0, normalize = false,
        minHz = 20, maxHz = 20000, points = 1200, floorDb = -180,
    } = {}) {
        if (!Number.isInteger(channel) || channel < 0 || channel >= meta.channels)
            throw new RangeError('channel must be a valid zero-based channel index');
        if (![0, 3, 6, 12, 24, 48].includes(smoothing))
            throw new RangeError('smoothing: 0 (off), 3, 6, 12, 24 or 48');
        if (typeof normalize !== 'boolean') throw new TypeError('normalize must be boolean');
        if (!Number.isFinite(minHz) || !Number.isFinite(maxHz) || minHz <= 0 ||
            minHz >= Math.min(maxHz, meta.rate / 2))
            throw new RangeError('Need 0 < minHz < min(maxHz, Nyquist)');
        if (!Number.isInteger(points) || points < 32 || points > 10000)
            throw new RangeError('points must be an integer from 32 to 10000');
        if (!Number.isFinite(floorDb) || floorDb >= 0)
            throw new RangeError('floorDb must be negative');
        const fft = spectrumFor(channel);
        const curve = IRCore.viewCurve(fft.power, meta.rate, fft.n, smoothing, points, minHz, maxHz);
        // Normalize AFTER smoothing, within the displayed frequency band.
        // Silence stays at floorDb rather than becoming a false 0 dB response.
        let peak = -Infinity;
        for (const point of curve.xy) peak = Math.max(peak, point[1]);
        const offset = normalize && Number.isFinite(peak) ? peak : 0;
        return {
            points: curve.xy.map(([hz, db]) => [hz, Math.max(floorDb, db - offset)]),
            minHz: curve.fmin, maxHz: curve.fmax,
            sampleRate: meta.rate, fftSize: fft.n, binSpacingHz: curve.df,
            channel, smoothing, normalized: normalize,
            normalizationOffsetDb: offset, silent: !Number.isFinite(peak),
        };
    }
    return { meta: { ...meta }, response };
}

/** Render one or more responses. rangeDb changes the viewport, not the data. */
function draw(canvas, responseOrArray, {
    rangeDb = 60, topDb,
    colors = ['#0f8178', '#d17735', '#7862ba', '#3d77c4'],
    background = '#ffffff', foreground = '#30404d', grid = '#e3e9ee',
} = {}) {
    if (!Number.isFinite(rangeDb) || rangeDb <= 0) throw new RangeError('rangeDb must be positive');
    if (topDb !== undefined && !Number.isFinite(topDb)) throw new RangeError('topDb must be finite');
    const curves = Array.isArray(responseOrArray) ? responseOrArray : [responseOrArray];
    if (!curves.length || curves.some(c => !c || !c.points.length)) throw new Error('No curve data');
    const { minHz, maxHz } = curves[0];
    if (curves.some(c => c.minHz !== minHz || c.maxHz !== maxHz)) throw new Error('Curve frequency ranges must match');
    if (!colors.length) throw new Error('At least one curve color is required');
    const width = Math.max(280, canvas.clientWidth || 900);
    const height = Math.max(200, canvas.clientHeight || 420);
    const dpr = root.devicePixelRatio || 1;
    canvas.width = Math.round(width * dpr); canvas.height = Math.round(height * dpr);
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('2D canvas is unavailable');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = background; ctx.fillRect(0, 0, width, height);
    let peak = -Infinity;
    for (const c of curves) for (const [, db] of c.points) peak = Math.max(peak, db);
    const high = topDb ?? (curves.every(c => c.normalized && !c.silent) ? 0 : Math.ceil(peak / 10) * 10);
    const low = high - rangeDb;
    const area = { x: 60, y: 28, width: width - 80, height: height - 72 };
    const x = hz => area.x + Math.log(hz / minHz) / Math.log(maxHz / minHz) * area.width;
    const y = db => area.y + (high - db) / rangeDb * area.height;
    ctx.font = '12px sans-serif'; ctx.lineWidth = 1;
    const step = [3, 6, 10, 12, 20, 30, 60, 120, 240].find(s => rangeDb / s <= 8) || Math.ceil(rangeDb / 8);
    for (let db = Math.ceil(low / step) * step; db <= high; db += step) {
        const yy = y(db); ctx.strokeStyle = grid;
        ctx.beginPath(); ctx.moveTo(area.x, yy); ctx.lineTo(area.x + area.width, yy); ctx.stroke();
        ctx.fillStyle = foreground; ctx.textAlign = 'right'; ctx.fillText(String(db), area.x - 9, yy + 4);
    }
    let lastLabelX = -Infinity;
    for (const hz of [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000]) {
        if (hz < minHz || hz > maxHz) continue;
        const xx = x(hz); ctx.strokeStyle = grid;
        ctx.beginPath(); ctx.moveTo(xx, area.y); ctx.lineTo(xx, area.y + area.height); ctx.stroke();
        if (xx - lastLabelX > 37) {
            ctx.fillStyle = foreground;
            ctx.textAlign = hz === minHz ? 'left' : hz === maxHz ? 'right' : 'center';
            ctx.fillText(hz >= 1000 ? hz / 1000 + 'k' : String(hz), xx, area.y + area.height + 20);
            lastLabelX = xx;
        }
    }
    ctx.fillStyle = foreground; ctx.textAlign = 'left'; ctx.fillText('Gain / dB', area.x, 15);
    ctx.textAlign = 'right'; ctx.fillText('Frequency / Hz', area.x + area.width, height - 4);
    ctx.save(); ctx.beginPath(); ctx.rect(area.x, area.y, area.width, area.height); ctx.clip();
    curves.forEach((curve, i) => {
        ctx.strokeStyle = colors[i % colors.length]; ctx.lineWidth = 1.6;
        ctx.setLineDash(i % 2 ? [6, 3] : []); ctx.beginPath();
        curve.points.forEach(([hz, db], j) => j ? ctx.lineTo(x(hz), y(db)) : ctx.moveTo(x(hz), y(db)));
        ctx.stroke();
    });
    ctx.restore(); ctx.setLineDash([]);
    canvas.setAttribute('role', 'img');
    canvas.setAttribute('aria-label', `IR frequency response, ${minHz} to ${maxHz} Hz, ${low} to ${high} dB`);
    return {
        plotArea: area, minDb: low, maxDb: high,
        frequencyAt: px => minHz * (maxHz / minHz) ** ((px - area.x) / area.width),
        dbAt: py => high - (py - area.y) / area.height * rangeDb,
    };
}

function exportPNG(canvas) {
    return new Promise((resolve, reject) => canvas.toBlob(
        blob => blob ? resolve(blob) : reject(new Error('PNG export failed')), 'image/png'
    ));
}

const API = { loadIR, draw, exportPNG };
if (typeof module !== 'undefined' && module.exports) module.exports = API;
else root.IRViewerCore = API;
})(typeof globalThis !== 'undefined' ? globalThis : this);

