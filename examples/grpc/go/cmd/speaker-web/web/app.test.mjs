import test from 'node:test';
import assert from 'node:assert/strict';
import {createSession} from './session.mjs';
const tick=()=>new Promise(r=>setImmediate(r));
function result(revision=0,answers={}){return {state:{modelDigest:'digest',revision,answers,skipped:[]},snapshot:{flow:{path:['a','b'],nextQuestionId:'b'},visual:{targets:[]}}};}
test('serialized Apply uses latest revision and renders before Quote completes',async()=>{
  let active=0,max=0;const revisions=[],renders=[];
  const s=createSession({send:async(method,body)=>{if(method==='initial')return {result:result()};if(method==='quote')return new Promise(()=>{});active++;max=Math.max(max,active);revisions.push(body.event.expectedRevision);await tick();active--;return {result:result(body.state.revision+1,{b:body.event.value})};},changed:s=>renders.push(s.result?.state.revision),failed:e=>{throw e;},save:()=>{}});
  await s.restore(null);s.select('b','one');s.select('b',state=>state.answers.b+'two');await s.settle();assert.equal(max,1);assert.deepEqual(revisions,[0,1]);assert.equal(s.result.state.answers.b,'onetwo');assert.ok(renders.includes(2));
});
test('late Quote cannot overwrite revision or focus; navigation never mutates State',async()=>{
  const waiting=[];const s=createSession({send:async(method,body)=>{if(method==='initial')return {result:result()};if(method==='quote')return new Promise(resolve=>waiting.push({resolve,request:body.quote}));return {result:result(1,{b:'new'})};},changed:()=>{},failed:e=>{throw e;},save:()=>{}});
  await s.restore(null);const original=JSON.stringify(s.result.state);s.navigate('a');assert.equal(JSON.stringify(s.result.state),original);
  waiting[1].resolve({quote:{modelDigest:'digest',revision:0,focus:{question:{id:'a'}},price:{status:'range'}}});await tick();assert.equal(s.quote.focus.question.id,'a');waiting[0].resolve({quote:{modelDigest:'digest',revision:0,focus:{question:{id:'b'}}}});await tick();assert.equal(s.quote.focus.question.id,'a');
  await s.select('b','new');s.navigate('b');const last=waiting.at(-1);last.resolve({quote:{modelDigest:'digest',revision:1,focus:{question:{id:'b'}}}});await tick();waiting[2].resolve({quote:{modelDigest:'digest',revision:1,focus:{question:{id:'a'}}}});await tick();assert.equal(s.quote.focus.question.id,'b');
});
test('Apply failure preserves successful State and recovery does not replace failed saved State',async()=>{
 let initial=0;const errors=[];const s=createSession({send:async(method)=>{if(method==='initial'){initial++;return {result:result()};}if(method==='quote')return new Promise(()=>{});throw new Error('invalid');},changed:()=>{},failed:e=>errors.push(e.message),save:()=>{}});
 await s.restore(null);await s.select('b','bad');assert.equal(s.result.state.revision,0);assert.deepEqual(errors,['invalid']);await assert.rejects(s.restore({revision:99}));assert.equal(initial,1);
});
test('Quote failure settles as failed while the committed Visual is retained',async()=>{
 const errors=[];const s=createSession({send:async(method,body)=>{if(method==='quote')throw new Error('quote failed');return {result:result(method==='apply'?body.state.revision+1:0)};},changed:()=>{},failed:e=>errors.push(e.message),save:()=>{}});await s.restore(null);await tick();assert.equal(s.quoteStatus,'failed');await s.select('b','new');await tick();assert.equal(s.result.state.revision,1);assert.equal(s.quoteStatus,'failed');assert.equal(s.quote,null);assert.equal(errors.length,2);
});
