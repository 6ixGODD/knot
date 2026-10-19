import {createSession} from './session.mjs';
const $=id=>document.getElementById(id),storageKey='knot-speaker-v1';
const labels={speaker_type:'音箱类型',material:'箱体材质',panel_color:'面板颜色',power:'输出功率',extras:'附加功能',cable:'接口类型'};
const values={active:'有源音箱',passive:'无源音箱',aluminum:'铝合金',walnut:'胡桃木','200w':'200W','100w':'100W','50w':'50W',led:'LED 灯环',bluetooth:'蓝牙模块',banana:'香蕉插头',spade:'叉形端子'};
const money=(n,c)=>new Intl.NumberFormat('zh-CN',{style:'currency',currency:c,maximumFractionDigits:2}).format(n/100);
function priceText(p){if(!p)return '报价中…';if(p.status==='no_completion')return '无法完成此配置';if(p.status==='unavailable')return '暂无法报价';return p.minMinor===p.maxMinor?money(p.minMinor,p.currency):money(p.minMinor,p.currency)+' – '+money(p.maxMinor,p.currency);}
function node(tag,text,cls){const e=document.createElement(tag);if(text!==undefined)e.textContent=text;if(cls)e.className=cls;return e;}
function button(text,action,cls){const e=node('button',text,cls);e.type='button';e.onclick=action;return e;}
function failed(e){$('error').hidden=false;$('error').textContent=e.message;}
async function send(method,body={},signal){const response=await fetch('/api/'+method,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body),signal});const payload=await response.json();$('raw').textContent=JSON.stringify({method,request:body,response:payload},null,2);$('request-info').textContent=method.toUpperCase();if(!response.ok||payload.transportError)throw new Error('服务调用失败：'+(payload.transportError||response.status));if(payload.error)throw new Error(payload.error.error.code+'：'+payload.error.error.message);$('connection').textContent='服务已连接';return payload;}
const session=createSession({send,changed:render,failed,save:state=>{try{localStorage.setItem(storageKey,JSON.stringify(state));}catch{}}});
let displayedQuestion=null;
function select(q,value){$('error').hidden=true;session.select(q.id,value,q.type!=='single');}
function question(focus){
  const area=$('question'),q=focus?.question;
  if(q?.type==='color'&&displayedQuestion===q.id&&area.querySelector('input[type=color]')) {const input=area.querySelector('input[type=color]');if(focus.answer&&document.activeElement!==input){input.value=focus.answer;area.querySelector('.color-row span').textContent=focus.answer;}if(focus.answer)area.querySelector('[data-use-color]')?.remove();area.querySelector('.candidate-price').textContent='选择后总价：'+priceText(session.quote?.price);return;}
  displayedQuestion=q?.id;area.replaceChildren();
  if(!q){area.append(node('h2',session.focus?'读取选项…':'配置完成'));return;}
  area.append(node('h2',labels[q.id]||q.label));
  if(q.type==='single'||q.type==='multi'){
    const options=node('div',undefined,'options');
    for(const c of focus.candidates){const selected=q.type==='multi'?(focus.answer||[]).includes(c.optionId):focus.answer===c.optionId;
      const option=q.options.find(o=>o.id===c.optionId);const el=button('',()=>select(q,q.type==='multi'?state=>{const old=state.answers[q.id]||[];return q.options.filter(o=>o.id===c.optionId?!old.includes(o.id):old.includes(o.id)).map(o=>o.id);}:c.candidateValue),'option'+(selected?' selected':''));
      el.dataset.optionId=c.optionId;el.disabled=!c.enabled||!!c.error;el.setAttribute('aria-pressed',String(selected));const copy=node('span',values[c.optionId]||option.label);
      copy.append(node('small',!c.enabled?'当前不可选':c.error?'此选择无效：'+c.error.error.code:'选择后总价：'+priceText(c.price)));
      el.append(copy,node('span',selected?'✓':'','option-mark'));options.append(el);
    }area.append(options);
    if(!q.required&&!Object.hasOwn(session.result.state.answers,q.id)&&!session.result.state.skipped.includes(q.id))area.append(button('跳过此项',()=>session.select(q.id,null,false,'skip_answer')));
  }else if(q.type==='color'){
    const row=node('div',undefined,'color-row'),input=node('input'),hex=node('span',focus.answer||'选择颜色');input.type='color';input.value=focus.answer||'#315a79';input.setAttribute('aria-label','面板颜色');
    input.oninput=()=>{hex.textContent=input.value.toUpperCase();select(q,input.value.toUpperCase());};input.onchange=()=>{hex.textContent=input.value.toUpperCase();select(q,input.value.toUpperCase());};row.append(input,hex);area.append(row,node('p','选择后总价：'+priceText(session.quote?.price),'candidate-price'));
    if(!focus.answer){const use=button('使用此颜色',()=>select(q,input.value.toUpperCase()));use.dataset.useColor='true';area.append(use);}
  }else {const input=node('input');input.type=q.type==='integer'?'number':'text';input.value=focus.answer??'';input.setAttribute('aria-label',q.label);input.onchange=()=>select(q,q.type==='integer'?Number(input.value):input.value);area.append(input);}
}
function render(s){
  if(!s.result)return;const {state,snapshot}=s.result,path=snapshot.flow.path;
  $('revision').textContent='Revision '+state.revision+(s.pending?' · 保存中':'');$('reset').disabled=!!s.pending;
  const targets=Object.fromEntries(snapshot.visual.targets.map(t=>[t.id,t.properties])),cabinet=targets['speaker.cabinet'],panel=targets['speaker.front_panel'],led=targets['speaker.led_ring'];
  $('speaker').classList.toggle('walnut',cabinet?.assetRef==='cabinet-walnut');
  for(const [layer,target] of [['base-layer','speaker.base'],['cabinet-layer','speaker.cabinet'],['panel-layer','speaker.front_panel']])$(layer).hidden=targets[target]?.visible===false;
  $('speaker').style.setProperty('--panel-color',panel?.color||'#26272B');$('speaker').style.setProperty('--led-color',led?.['emission.color']||'#315A79');$('led-layer').hidden=!led?.visible;$('led-layer').style.opacity=led?.['emission.strength']??1;
  $('material-label').textContent=cabinet?.assetRef==='cabinet-walnut'?'胡桃木':cabinet?.assetRef==='cabinet-aluminum'?'铝合金':'箱体预览';
  $('price').textContent=s.quoteStatus==='failed'?'报价失败':priceText(s.quote?.price);$('price-status').textContent=s.quote?.price.status==='final'?'完整配置总价':'完整配置价格范围';$('price-note').textContent=s.quote?.price.status==='unavailable'?'超出精确报价范围，可继续配置后重试。':s.quote?.price.status==='no_completion'?'请返回修改已有选项。':'';
  $('steps').replaceChildren(...path.map(id=>button(labels[id]||id,()=>{displayedQuestion=null;s.navigate(id);},id===s.focus?'active':'')));
  if(s.quote)question(s.quote.focus);else if(displayedQuestion!==s.focus){displayedQuestion=null;question(null);}else {
    const status=s.quoteStatus==='failed'?'报价失败':'报价中…';
    for(const el of $('question').querySelectorAll('[data-option-id]')) {const answer=state.answers[s.focus],selected=Array.isArray(answer)?answer.includes(el.dataset.optionId):answer===el.dataset.optionId;el.classList.toggle('selected',selected);el.setAttribute('aria-pressed',String(selected));el.querySelector('.option-mark').textContent=selected?'✓':'';if(!el.disabled)el.querySelector('small').textContent='选择后总价：'+status;}
    const price=$('question').querySelector('.candidate-price');if(price)price.textContent='选择后总价：'+status;
  }
  const index=path.indexOf(s.focus);$('back').disabled=!!s.pending||(index<=0&&s.focus!==null);$('back').onclick=()=>{displayedQuestion=null;s.navigate(s.focus===null?path.at(-1):path[index-1]);};
  const handled=s.focus===null||Object.hasOwn(state.answers,s.focus)||state.skipped.includes(s.focus);$('next').disabled=!!s.pending||!handled||s.focus===null;
  $('next').textContent=index===path.length-1&&snapshot.flow.status==='complete'?'完成':'下一项';$('next').onclick=()=>{displayedQuestion=null;s.navigate(path[index+1]??null);};
}
$('reset').onclick=async()=>{displayedQuestion=null;$('error').hidden=true;try{await session.restore(null);}catch(e){failed(e);}};
async function boot(){try{
  for(const name of ['base','cabinet','panel','led']){const response=await fetch(name+'.svg');if(!response.ok)throw new Error('无法加载图层');const svg=new DOMParser().parseFromString(await response.text(),'image/svg+xml').documentElement;if(svg.localName!=='svg')throw new Error('无效图层');$(name+'-layer').append(document.importNode(svg,true));}
  let saved=null;const raw=localStorage.getItem(storageKey);if(raw)saved=JSON.parse(raw);await session.restore(saved);
}catch(e){failed(e);$('connection').textContent='恢复失败，请检查诊断或重新开始';}}
boot();
