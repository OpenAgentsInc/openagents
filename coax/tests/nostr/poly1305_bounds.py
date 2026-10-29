#!/usr/bin/env python3
"""Optional Z3 check of the hand-authored carry model, not source equivalence."""
if not __debug__: raise SystemExit('tests require Python assertions')
import hashlib
import subprocess
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/crypto/poly1305.coax'
SOURCE_HASH='260530313c303e2d06a962471eae3d0b5b5168507826fee12d163cf7f7e48e23'
BASE=1<<26
LIMIT=1<<58


def query(rounds):
    lines=['(set-logic QF_LIA)','(set-option :timeout 30000)']
    for i in range(5):
        lines += [f'(declare-const x{i} Int)',f'(assert (and (<= 0 x{i}) (< x{i} {LIMIT})))']
    values=[f'x{i}' for i in range(5)]
    for cycle in range(rounds):
        for i in range(5):
            carry=f'c{cycle}_{i}';low=f'l{cycle}_{i}'
            lines += [f'(define-fun {carry} () Int (div {values[i]} {BASE}))',f'(define-fun {low} () Int (mod {values[i]} {BASE}))']
            values[i]=low;target=0 if i==4 else i+1;factor=5 if i==4 else 1
            name=f'a{cycle}_{i}'
            lines.append(f'(define-fun {name} () Int (+ {values[target]} (* {factor} {carry})))')
            values[target]=name
    lines += ['(assert (or '+' '.join(f'(>= {v} {BASE})' for v in values)+'))','(check-sat)']
    return '\n'.join(lines)+'\n'


def main():
    assert hashlib.sha256(SOURCE.read_bytes()).hexdigest()==SOURCE_HASH, 'source changed: review the model binding'
    assert 25*(1<<27)*(1<<26)<LIMIT
    for rounds,expected in [(3,'unsat'),(2,'sat')]:
        result=subprocess.run(['z3','-in'],input=query(rounds),text=True,capture_output=True,check=True).stdout.strip()
        assert result==expected,(rounds,result)
        print(rounds,'carry rounds:',result)


if __name__=='__main__': main()
