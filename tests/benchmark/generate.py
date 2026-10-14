"""Deterministic 100-question / 500-condition / 100-property / 100-component model."""
def model():
    def condition(field): return dict(field=field,op='eq',value='a')
    questions=[]
    for i in range(100):
        options=[dict(id=value,label=value) for value in 'abcde']
        if i:
            for option in options[1:]: option['enabledWhen']=condition(f'q{i-1}')
        questions.append(dict(id=f'q{i}',type='single',label=f'Question {i}',required=True,options=options))
    edges=[dict(**{'from':f'q{i}','to':f'q{i+1}'},when=condition(f'q{i}')) for i in range(99)]
    edges.append({'from':'q99','to':'END'})
    targets=[dict(id=f't{i}',order=i,defaults=dict(opacity=1)) for i in range(100)]
    slots=[dict(id=f's{i}',target=f't{i}',property='opacity',cases=[dict(when=condition(f'q{i}'),value=0.5)]) for i in range(5)]
    components=[dict(id=f'p{i}',category='base' if i==0 else 'adjustment',amountMinor=100) for i in range(100)]
    result=dict(schemaVersion='1.0',productId='benchmark-100',productVersion='1',questions=questions,flow=dict(start='q0',edges=edges),pricing=dict(currency='USD',components=components),visual=dict(targets=targets,slots=slots))
    assert 99*4+99+5 == 500
    return result


if __name__ == '__main__':
    import argparse
    import json
    from pathlib import Path
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out',type=Path,required=True)
    args=parser.parse_args();args.out.mkdir(parents=True,exist_ok=True)
    (args.out/'model.json').write_text(json.dumps(model()),encoding='utf-8')
    events=[dict(schemaVersion='1.0',type='set_answer',questionId=f'q{i}',expectedRevision=i,value='a') for i in range(100)]
    (args.out/'events.json').write_text(json.dumps(events),encoding='utf-8')
