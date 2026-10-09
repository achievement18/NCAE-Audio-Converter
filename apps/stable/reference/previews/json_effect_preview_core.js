/* MIT License
 * Copyright (c) 2026 JSON Effect Viewer contributors
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
 * of the Software, and to permit persons to whom the Software is furnished to do
 * so, subject to the following conditions: The above copyright notice and this
 * permission notice shall be included in all copies or substantial portions of
 * the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO MERCHANTABILITY, FITNESS FOR
 * A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
 * COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
 * IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
 * CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 *
 * NCAE JSON adapters use actual rvb/rotate/eq.eqs/peq.f sample structures.
 * Private numeric PEQ types are NEVER assigned a guessed filter type.
 * DSP models: independent implementation of W3C Audio EQ Cookbook equations.
 */
(function (root) {
'use strict';
const DEFAULT_FREQUENCIES = Object.freeze([31,62,125,250,500,1000,2000,4000,8000,16000]);
const TYPES = new Set(['peaking','lowshelf','highshelf','lowpass','highpass','notch','bandpass','allpass']);
const obj = x => x !== null && typeof x === 'object' && !Array.isArray(x);
const finite = x => typeof x === 'number' && Number.isFinite(x);
const own = (x,k) => Object.prototype.hasOwnProperty.call(x,k);
function state(x) { return x == null ? 'missing' : !obj(x) ? 'unknown' : x.on === true ? 'on' : x.on === false ? 'off' : 'unknown'; }
function readJSON(input) {
    let text = typeof input === 'string' ? input : JSON.stringify(input);
    if (typeof text !== 'string' || text.length > 1024*1024) throw Error('JSON 必须是对象，且不超过 1 MiB 字符。');
    const value = JSON.parse(text.replace(/^\uFEFF/,''));
    if (!obj(value)) throw Error('顶层必须是 JSON 对象。');
    let count=0;
    function check(x,depth) {
        if (++count > 20000 || depth > 30) throw Error('JSON 节点数或嵌套深度超过限制。');
        if (typeof x === 'number' && !Number.isFinite(x)) throw Error('JSON 含非有限数值。');
        if (obj(x) || Array.isArray(x)) for (const v of Object.values(x)) check(v,depth+1);
    }
    check(value,0); return value;
}
function leaves(value, path='', output=[]) {
    if (obj(value)) for (const [key,v] of Object.entries(value)) leaves(v,path?path+'.'+key:key,output);
    else output.push({path,value});
    return output;
}

/** Standard RBJ biquad; this does NOT determine a private numeric type code. */
function biquad({type,frequency,gainDb=0,q=1},sampleRate) {
    if (!TYPES.has(type)) throw Error('不支持的滤波器类型：'+type);
    if (!finite(sampleRate) || sampleRate < 100 || sampleRate > 768000) throw Error('采样率无效。');
    if (!finite(frequency) || frequency<=0 || frequency>=sampleRate/2) throw Error('滤波频率必须位于 0 与奈奎斯特频率之间。');
    if (!finite(q) || q<0.0001 || q>10000 || !finite(gainDb) || Math.abs(gainDb)>120) throw Error('Q 或增益超出模型范围。');
    const w=2*Math.PI*frequency/sampleRate,c=Math.cos(w),s=Math.sin(w),alpha=s/(2*q),A=10**(gainDb/40),beta=2*Math.sqrt(A)*alpha;
    let b,a;
    switch(type) {
      case 'peaking': b=[1+alpha*A,-2*c,1-alpha*A];a=[1+alpha/A,-2*c,1-alpha/A];break;
      case 'lowpass': b=[(1-c)/2,1-c,(1-c)/2];a=[1+alpha,-2*c,1-alpha];break;
      case 'highpass': b=[(1+c)/2,-(1+c),(1+c)/2];a=[1+alpha,-2*c,1-alpha];break;
      case 'notch': b=[1,-2*c,1];a=[1+alpha,-2*c,1-alpha];break;
      case 'bandpass': b=[alpha,0,-alpha];a=[1+alpha,-2*c,1-alpha];break;
      case 'allpass': b=[1-alpha,-2*c,1+alpha];a=[1+alpha,-2*c,1-alpha];break;
      case 'lowshelf':
        b=[A*((A+1)-(A-1)*c+beta),2*A*((A-1)-(A+1)*c),A*((A+1)-(A-1)*c-beta)];
        a=[(A+1)+(A-1)*c+beta,-2*((A-1)+(A+1)*c),(A+1)+(A-1)*c-beta];break;
      case 'highshelf':
        b=[A*((A+1)+(A-1)*c+beta),-2*A*((A-1)+(A+1)*c),A*((A+1)+(A-1)*c-beta)];
        a=[(A+1)-(A-1)*c+beta,2*((A-1)-(A+1)*c),(A+1)-(A-1)*c-beta];break;
    }
    return {b:b.map(v=>v/a[0]),a:a.map(v=>v/a[0]),type,frequency,gainDb,q};
}
function gainAt(filter,frequency,sampleRate) {
    const w=2*Math.PI*frequency/sampleRate,c=Math.cos(w),s=Math.sin(w),c2=Math.cos(2*w),s2=Math.sin(2*w);
    const magnitude=v=>Math.hypot(v[0]+v[1]*c+v[2]*c2,-v[1]*s-v[2]*s2);
    return 20*Math.log10(Math.max(Number.MIN_VALUE,magnitude(filter.b))/Math.max(Number.MIN_VALUE,magnitude(filter.a)));
}
function interpolate(controls,f) {
    if(f<=controls[0][0])return controls[0][1];
    if(f>=controls.at(-1)[0])return controls.at(-1)[1];
    let i=1;while(controls[i][0]<f)i++;
    const [f0,g0]=controls[i-1],[f1,g1]=controls[i];
    return g0+(g1-g0)*Math.log(f/f0)/Math.log(f1/f0);
}

/** Parse actual preset structures and build clearly labelled visualization data.
 * Defaults: ten-band control-point preview, NO numeric PEQ type assumptions.
 */
function inspect(input,options={}) {
    const raw=readJSON(input),warnings=[],modules={};
    for(const key of ['rvb','rotate','eq','peq','bt','se','cmp','limiter']) {
        modules[key]={state:state(raw[key]),raw:raw[key]??null,parameters:obj(raw[key])?leaves(raw[key]):[]};
    }
    const sampleRate=options.sampleRate??48000,minHz=options.minHz??20,maxHz=Math.min(options.maxHz??20000,sampleRate/2*(1-1e-9));
    const count=options.points??720,mode=options.graphicMode??'controls';
    if(!finite(sampleRate)||sampleRate<100||sampleRate>768000||!finite(minHz)||!finite(maxHz)||minHz<=0||minHz>=maxHz)throw Error('频率范围或采样率无效。');
    if(!Number.isInteger(count)||count<32||count>10000)throw Error('points 必须为 32–10000 的整数。');
    if(!['controls','rbj'].includes(mode))throw Error('graphicMode 仅支持 controls 或 rbj。');
    if(mode==='rbj'&&(!finite(options.graphicQ)||options.graphicQ<=0))throw Error('RBJ 十段模型必须明确提供 graphicQ，不自动猜带宽。');
    const frequencies=options.eqFrequencies??DEFAULT_FREQUENCIES;
    if(!Array.isArray(frequencies)||!frequencies.length||frequencies.some((f,i)=>!finite(f)||f<=0||(i&&f<=frequencies[i-1])))throw Error('eqFrequencies 必须为递增正数数组。');
    const typeMap=options.peqTypeMap??{};
    if(!obj(typeMap))throw Error('peqTypeMap 必须是对象。');
    const qScale=options.peqQScale??1;
    if(!finite(qScale)||qScale<=0)throw Error('peqQScale 必须为正数。');
    const grid=Array.from({length:count},(_,i)=>minHz*(maxHz/minHz)**(i/(count-1)));
    const controls=[],markers=[],series=[],graphicFilters=[],peqFilters=[],bands=[];
    let graphicReady=modules.eq.state==='off'||modules.eq.state==='missing';
    let peqReady=modules.peq.state==='off'||modules.peq.state==='missing';
    let graphicCurve=null,peqCurve=null,preamp=0;
    const makeCurve=(label,fn,kind,extra={})=>({label,kind,points:grid.map(f=>[f,Math.max(-180,Math.min(180,fn(f)))]),minHz,maxHz,normalized:false,silent:false,...extra});
    if(modules.eq.state==='on') {
        const gains=raw.eq.eqs;
        if(!Array.isArray(gains)||gains.length!==frequencies.length||gains.some(v=>!finite(v)))warnings.push('eq.eqs 与中心频率数量不匹配或含非法值；原始值仍保留，未伪造曲线。');
        else {
            gains.forEach((g,i)=>controls.push([frequencies[i],g]));
            if(!options.eqFrequencies)warnings.push('十段中心频率采用常见展示假设：31/62/125/250/500/1k/2k/4k/8k/16k Hz；JSON 本身没有这些频点。');
            warnings.push('eq.eqs 数值按 dB 展示；连线是设置值参考，不等同于原播放器的实际滤波响应。');
            if(mode==='controls') {
                graphicCurve=makeCurve('十段 EQ 设置参考线（非真实频响）',f=>interpolate(controls,f),'controls');
                graphicReady=false;
            } else {
                try {
                    controls.forEach(([frequency,gainDb])=>graphicFilters.push(biquad({type:'peaking',frequency,gainDb,q:options.graphicQ},sampleRate)));
                    graphicCurve=makeCurve('十段 EQ：RBJ 模型估算',f=>graphicFilters.reduce((s,b)=>s+gainAt(b,f,sampleRate),0),'model');
                    graphicReady=true;warnings.push('十段 EQ 使用调用方指定的中心频率/Q 和标准峰值滤波器模型；不代表网易内部实现。');
                } catch(e) {warnings.push('十段 EQ 模型不可用：'+e.message);graphicReady=false;}
            }
            if(graphicCurve)series.push(graphicCurve);
        }
    }
    if(modules.peq.state==='on') {
        if(!Array.isArray(raw.peq.f))warnings.push('peq.f 缺失或不是数组，未绘制伪造 PEQ 频响。');
        else {
            peqReady=true;
            if(raw.peq.gain!==undefined&&!finite(raw.peq.gain)){peqReady=false;warnings.push('peq.gain 非法；没有默认当成 0 dB。');}
            else preamp=raw.peq.gain??0;
            raw.peq.f.forEach((band,index)=>{
                const row={index,state:state(band),raw:band,filter:null,error:null};bands.push(row);
                if(row.state==='off')return;
                if(row.state!=='on'){peqReady=false;row.error='频段开关状态未知';return;}
                const frequency=band.freq,gainDb=band.gain;
                if(finite(frequency)&&frequency>0&&finite(gainDb))markers.push({frequency,gainDb,band:band.band??index,type:band.type,label:'PEQ 参数点（不是频响上的实测点）'});
                const key=String(band.type);
                const type=own(typeMap,key)?typeMap[key]:(typeof band.type==='string'&&TYPES.has(band.type)?band.type:null);
                if(!type){peqReady=false;row.error='未映射类型码 '+key;return;}
                try {
                    const needsGain=['peaking','lowshelf','highshelf'].includes(type);
                    if(needsGain&&!finite(gainDb))throw Error('频段 gain 缺失或无效');
                    const gain=needsGain?gainDb:0;
                    row.filter=biquad({type,frequency,gainDb:gain,q:band.q*qScale},sampleRate);peqFilters.push(row.filter);
                } catch(e){peqReady=false;row.error=e.message;}
            });
            const invalid=bands.filter(b=>b.error);
            if(invalid.length)warnings.push('PEQ 有 '+invalid.length+' 段无法确认模型：'+invalid.map(b=>'#'+(b.index+1)+' '+b.error).join('；')+'。这些段仅展示参数，不纳入曲线。');
            if(peqReady||peqFilters.length) {
                peqCurve=makeCurve(peqReady?'PEQ：RBJ 模型估算':'PEQ：仅已解析部分（非完整响应）',f=>preamp+peqFilters.reduce((s,b)=>s+gainAt(b,f,sampleRate),0),'model',{complete:peqReady});
                series.push(peqCurve);warnings.push('PEQ 模型将 freq/gain/q 按 Hz/dB/调用方 Q 映射解释；数值类型码由调用方提供，不从样例猜测。');
            }
        }
    }
    if(modules.eq.state==='unknown'||modules.peq.state==='unknown')warnings.push('存在开关状态未知的 EQ 模块，不能判断完整 EQ 响应。');
    const active=modules.eq.state==='on'||modules.peq.state==='on';
    let combined=null;
    if(active&&graphicReady&&peqReady&&modules.eq.state!=='unknown'&&modules.peq.state!=='unknown') {
        combined=makeCurve('EQ + PEQ 合成模型（不含混响/旋转/音调）',f=>(modules.peq.state==='on'?preamp:0)+[...graphicFilters,...peqFilters].reduce((s,b)=>s+gainAt(b,f,sampleRate),0),'combined');
        if(graphicCurve&&peqCurve)series.push(combined);
    }
    if(!active&&modules.eq.state!=='unknown'&&modules.peq.state!=='unknown')series.push(makeCurve('无已开启 EQ：0 dB 参考线（不是整套效果）',()=>0,'bypass'));
    if(modules.bt.state==='on')warnings.push('bt 已开启：bass/treble 参数保留展示，但未纳入 EQ 曲线。');
    warnings.push('所有曲线只描述 EQ 设置或明确指定的模型，不包含 rvb/rotate 等效果的实际响应。');
    return {raw,modules,warnings,bands,eqView:{series,controls,markers,combined,minHz,maxHz,sampleRate,mode,complete:!!combined,status:active?(combined?'model':series.length?'preview':'unresolved'):(modules.eq.state==='unknown'||modules.peq.state==='unknown'?'unresolved':'inactive')}};
}

/** Minimal preview: EQ graph only. Parsed non-EQ data remains available in inspect().
 * Requires ir_viewer_core.js or an explicitly supplied drawCurve function.
 */
function render(container,model,{drawCurve,plotHeight=320,showPeqMarkers=false}={}) {
    const document=container.ownerDocument;
    const plotter=drawCurve??root.IRViewerCore?.draw;
    if(typeof plotter!=='function')throw Error('请先加载 ir_viewer_core.js，或传入 drawCurve。');
    const e=(tag,text,css)=>{const n=document.createElement(tag);if(text!==undefined)n.textContent=text;if(css)n.style.cssText=css;return n;};
    const wrapper=e('section',undefined,'font:13px/1.6 system-ui,sans-serif;color:inherit;');
    const canvas=e('canvas',undefined,'width:100%;height:'+plotHeight+'px;display:block;');
    wrapper.append(canvas);
    const palette=['#0f8178','#d17735','#7862ba','#3d77c4'];
    const markers=showPeqMarkers?model.eqView.markers:[];
    const legend=e('div',undefined,'display:flex;gap:14px;flex-wrap:wrap;font-size:12px;margin-top:6px;');
    model.eqView.series.forEach((curve,i)=>{const label=e('span',curve.label);label.style.color=palette[i%palette.length];legend.append(label);});
    if(markers.length)legend.append(e('span','◇ PEQ 参数点（非滤波响应）'));
    wrapper.append(legend);
    const notes=[];
    if(model.eqView.controls.length&&model.eqView.mode==='controls')notes.push('设置参考线，不是实测频响');
    if(model.warnings.some(w=>w.includes('展示假设')))notes.push('十段中心频率为展示假设');
    const unresolved=model.bands.filter(b=>b.error).length;
    if(unresolved)notes.push('PEQ 有 '+unresolved+' 段未解析，未计入曲线');
    if(!model.eqView.series.length)notes.push('当前没有可计算的 EQ 曲线');
    if(model.eqView.status==='inactive')notes.push('EQ 未启用；0 dB 线不代表整套音效');
    else notes.push('不含混响、旋转和 bt 音调效果');
    wrapper.append(e('div',notes.join(' · '),'font-size:11px;opacity:.72;margin-top:5px;'));
    container.replaceChildren(wrapper);
    function redraw() {
        const curves=model.eqView.series.length?model.eqView.series:[{points:[[model.eqView.minHz,0],[model.eqView.maxHz,0]],minHz:model.eqView.minHz,maxHz:model.eqView.maxHz,normalized:false,silent:false}];
        let lo=-12,hi=12;
        for(const c of model.eqView.series)for(const [,v] of c.points){lo=Math.min(lo,v-3);hi=Math.max(hi,v+3);}
        for(const p of markers){lo=Math.min(lo,p.gainDb-3);hi=Math.max(hi,p.gainDb+3);}
        lo=Math.floor(lo/6)*6;hi=Math.ceil(hi/6)*6;
        const mapping=plotter(canvas,curves,{topDb:hi,rangeDb:hi-lo,colors:model.eqView.series.length?palette:['#8880']});
        const ctx=canvas.getContext('2d'),a=mapping.plotArea;
        const x=f=>a.x+Math.log(f/model.eqView.minHz)/Math.log(model.eqView.maxHz/model.eqView.minHz)*a.width;
        const y=db=>a.y+(hi-db)/(hi-lo)*a.height;
        ctx.save();ctx.beginPath();ctx.rect(a.x,a.y,a.width,a.height);ctx.clip();
        if(model.modules.eq.state==='on')for(const [f,g] of model.eqView.controls){if(f<model.eqView.minHz||f>model.eqView.maxHz)continue;ctx.fillStyle=palette[0];ctx.beginPath();ctx.arc(x(f),y(g),3.2,0,2*Math.PI);ctx.fill();}
        ctx.strokeStyle=palette[1];ctx.lineWidth=1.6;
        for(const point of markers){if(point.frequency<model.eqView.minHz||point.frequency>model.eqView.maxHz)continue;const px=x(point.frequency),py=y(point.gainDb);ctx.beginPath();ctx.moveTo(px,py-5);ctx.lineTo(px+5,py);ctx.lineTo(px,py+5);ctx.lineTo(px-5,py);ctx.closePath();ctx.stroke();}
        ctx.restore();
        if(!model.eqView.series.length){ctx.fillStyle='#57616a';ctx.font='13px sans-serif';ctx.textAlign='center';ctx.fillText('没有可绘制的 EQ 曲线',a.x+a.width/2,a.y+a.height/2);}
        canvas.setAttribute('aria-label','EQ 预览：'+(model.eqView.series.map(s=>s.label).join('；')||'模型未解析')+'。不代表整套音效真实响应。');
    }
    redraw();
    const Observer=document.defaultView?.ResizeObserver;const observer=Observer?new Observer(redraw):null;observer?.observe(canvas);
    return {canvas,redraw,destroy(){observer?.disconnect();wrapper.remove();}};
}
const API={inspect,render,biquad,gainAt,DEFAULT_FREQUENCIES};
if(typeof module!=='undefined'&&module.exports)module.exports=API;else root.JsonEffectPreviewCore=API;
})(typeof globalThis!=='undefined'?globalThis:this);



