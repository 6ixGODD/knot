// State mutations are serialized; Quote responses are bound to all input identity.
export function createSession({send, changed, failed, save}) {
  const session={result:null,focus:null,quote:null,quoteStatus:"idle",pending:0};
  let sequence=0, controller=null, tail=Promise.resolve();
  const identity=()=>JSON.stringify([session.result?.state.modelDigest,session.result?.state.revision,session.focus]);
  function invalidate(){sequence++;controller?.abort();session.quote=null;session.quoteStatus="loading";}
  async function refreshQuote(){
    if(!session.result)return;
    invalidate();const seq=sequence,key=identity(),state=session.result.state;
    controller=new AbortController();const signal=controller.signal;
    const request={schemaVersion:'1.0',state};if(session.focus)request.focusQuestionId=session.focus;
    changed(session);
    try {const payload=await send('quote',{quote:request},signal);
      if(seq!==sequence||key!==identity())return;
      const q=payload.quote;if(!q||q.modelDigest!==state.modelDigest||q.revision!==state.revision||(session.focus&&q.focus?.question.id!==session.focus))throw new Error('报价身份不匹配');
      session.quote=q;session.quoteStatus="settled";changed(session);
    }catch(e){if(seq===sequence&&e.name!=='AbortError'){session.quoteStatus='failed';changed(session);failed(e);}}
  }
  function accept(result, keepFocus){
    session.result=result;
    const path=result.snapshot.flow.path;
    if(!keepFocus || !path.includes(session.focus))session.focus=result.snapshot.flow.nextQuestionId;
    save(result.state);changed(session); // Visual is rendered before Quote starts.
    void refreshQuote();
  }
  session.restore=async(state)=>{invalidate();const payload=await send(state?'evaluate':'initial',state?{state}:{});accept(payload.result,false);};
  session.navigate=(focus)=>{if(focus!==null&&!session.result.snapshot.flow.path.includes(focus))return;session.focus=focus;void refreshQuote();};
  session.select=(questionId,value,keepFocus=true,type='set_answer')=>{
    invalidate();session.pending++;changed(session);
    const task=tail.then(async()=>{
      const state=session.result.state,event={schemaVersion:'1.0',type,questionId,expectedRevision:state.revision};
      if(type==='set_answer')event.value=typeof value==='function'?value(state):value;
      const payload=await send('apply',{state,event});accept(payload.result,keepFocus);
    });
    tail=task.catch(e=>{failed(e);void refreshQuote();}).finally(()=>{session.pending--;changed(session);});return tail;
  };
  session.settle=()=>tail;
  return session;
}
