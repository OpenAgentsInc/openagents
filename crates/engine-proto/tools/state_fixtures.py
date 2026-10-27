"""Synthetic state fixtures exercise registered types across languages."""
import json


def generate(output):
    from openagents.engine.v1.state_pb2 import WorldSnapshot
    folder=output/'fixtures/state';folder.mkdir(parents=True)
    snapshot=WorldSnapshot(contract_version=1,world='wire-fixture',schema=1,revision=7)
    definition=snapshot.definitions.add(name='Fixture',version=1)
    for name,kind,bits in [('enabled',1,0),('score',2,32),('coins',3,32),('weight',4,64),('label',5,0),('position',6,32),('target',7,32)]:
        definition.fields.add(name=name,kind=kind,bits=bits)
    component=snapshot.entities.add(id=0).components.add(name='Fixture',version=1)
    component.fields.add(name='enabled',kind=1,boolean=False)
    component.fields.add(name='score',kind=2,integer=-7)
    component.fields.add(name='coins',kind=3,unsigned_value=4294967295)
    component.fields.add(name='weight',kind=4,number=1/3)
    component.fields.add(name='label',kind=5,text='synthetic UTF-8 π')
    component.fields.add(name='position',kind=6,vector=[1,2,3])
    component.fields.add(name='target',kind=7,entity=4294967295)
    cases={}
    def write(name,value,valid):
        (folder/name).write_bytes(value.SerializeToString(deterministic=True));cases[name]=valid
    write('snapshot.pb',snapshot,True)
    invalid=WorldSnapshot();invalid.CopyFrom(snapshot);invalid.entities.add().CopyFrom(snapshot.entities[0]);write('duplicate-entity.pb',invalid,False)
    invalid.CopyFrom(snapshot);invalid.entities[0].components[0].fields[0].kind=4;write('wrong-field-kind.pb',invalid,False)
    invalid.CopyFrom(snapshot);invalid.entities[0].components[0].fields[1].integer=2147483648;write('integer-overflow.pb',invalid,False)
    invalid.CopyFrom(snapshot);invalid.entities[0].components[0].fields[3].number=float('nan');write('nan.pb',invalid,False)
    (folder/'unknown.pb').write_bytes(snapshot.SerializeToString()+b'\x98\x06\x01');cases['unknown.pb']=False
    (folder/'cases.json').write_text(json.dumps(cases,indent=2)+'\n')
