#!/usr/bin/env python3
"""Firm reason2: independent NumPy analytic jets and smooth finite differences.

No Coax implementation is imported. Each case has a separate mathematical
expression; formal derivatives propagate in F64 at F32 stored primal values.
Finite differences are test-only and never produce runtime derivatives.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import numpy as np

ANALYTIC_ATOL = ANALYTIC_RTOL = 2e-11
FD_J_ATOL = FD_J_RTOL = 5e-5
FD_H_ATOL = FD_H_RTOL = 2e-4

class Jet:
    def __init__(self, v, j, h):
        self.v = np.asarray(v, dtype=np.float32)
        self.j, self.h = np.asarray(j, dtype=np.float64), np.asarray(h, dtype=np.float64)
        self.d = self.j.shape[-1]
    def __add__(self, other):
        shape = np.broadcast_shapes(self.v.shape, other.v.shape)
        return Jet(np.float32(self.v + other.v), np.broadcast_to(self.j, shape+(self.d,))+np.broadcast_to(other.j, shape+(self.d,)), np.broadcast_to(self.h, shape+(self.d,self.d))+np.broadcast_to(other.h, shape+(self.d,self.d)))
    def __mul__(self, other):
        shape = np.broadcast_shapes(self.v.shape, other.v.shape)
        a, b = np.broadcast_to(self.v, shape).astype(np.float64), np.broadcast_to(other.v, shape).astype(np.float64)
        aj, bj = np.broadcast_to(self.j, shape+(self.d,)), np.broadcast_to(other.j, shape+(self.d,))
        ah, bh = np.broadcast_to(self.h, shape+(self.d,self.d)), np.broadcast_to(other.h, shape+(self.d,self.d))
        return Jet(np.float32(self.v*other.v), aj*b[...,None]+a[...,None]*bj, ah*b[...,None,None]+a[...,None,None]*bh+aj[..., :,None]*bj[...,None,:]+bj[..., :,None]*aj[...,None,:])
    def relu(self):
        mask = self.v > 0
        return Jet(np.where(mask, self.v, np.float32(0)), self.j*mask[...,None], self.h*mask[...,None,None])
    def sum(self, axis):
        v = np.zeros(self.v.shape[:axis]+self.v.shape[axis+1:], dtype=np.float32)
        for k in range(self.v.shape[axis]):
            v = np.float32(v+np.take(self.v,k,axis=axis))
        return Jet(v,self.j.sum(axis=axis),self.h.sum(axis=axis))
    def take(self, indices, axis):
        return Jet(np.take(self.v,indices,axis=axis),np.take(self.j,indices,axis=axis),np.take(self.h,indices,axis=axis))
    def reshape(self, shape):
        return Jet(self.v.reshape(shape),self.j.reshape(shape+(self.d,)),self.h.reshape(shape+(self.d,self.d)))
    def matmul(self, other):
        m,k = self.v.shape
        n = other.v.shape[1]
        v,j,h = np.zeros((m,n),np.float32),np.zeros((m,n,self.d)),np.zeros((m,n,self.d,self.d))
        for a in range(m):
            for b in range(n):
                for c in range(k):
                    va,vb = float(self.v[a,c]),float(other.v[c,b])
                    ja,jb = self.j[a,c],other.j[c,b]
                    v[a,b] = np.float32(v[a,b]+np.float32(self.v[a,c]*other.v[c,b]))
                    j[a,b] += ja*vb+va*jb
                    h[a,b] += self.h[a,c]*vb+va*other.h[c,b]+np.outer(ja,jb)+np.outer(jb,ja)
        return Jet(v,j,h)
    def norm(self, epsilon):
        width = self.v.shape[-1]
        rows = self.v.reshape(-1,width)
        js,hs = self.j.reshape(-1,width,self.d),self.h.reshape(-1,width,self.d,self.d)
        values,grads,hess = [],[],[]
        p = np.eye(width)-np.ones((width,width))/width
        for row,j,h in zip(rows,js,hs):
            # The primal is explicitly F32, including each reduction step.
            total=np.float32(0)
            for value in row:total=np.float32(total+value)
            mean=np.float32(total/np.float32(width));squares=np.float32(0)
            for value in row:
                delta=np.float32(value-mean);squares=np.float32(squares+np.float32(delta*delta))
            variance=np.float32(np.float32(squares/np.float32(width))+np.float32(epsilon))
            denominator=np.float32(math.sqrt(float(variance)))
            values.append(np.float32(np.float32(row-mean)/denominator))
            c=row.astype(np.float64)-row.astype(np.float64).mean()
            r=1/math.sqrt(np.dot(c,c)/width+float(np.float32(epsilon)))
            local_j=r*p-r**3*np.outer(c,c)/width
            local_h=np.empty((width,width,width))
            for a in range(width):
                for b in range(width):
                    for k in range(width):
                        local_h[a,b,k]=-r**3*(p[a,b]*c[k]+p[a,k]*c[b]+c[a]*p[b,k])/width+3*r**5*c[a]*c[b]*c[k]/width**2
            grads.append(local_j@j)
            hess.append(np.einsum('ij,jab->iab',local_j,h)+np.einsum('ijk,ja,kb->iab',local_h,j,j))
        return Jet(np.array(values).reshape(self.v.shape),np.array(grads).reshape(self.j.shape),np.array(hess).reshape(self.h.shape))

def jets(case,x):
    x=np.asarray(x,np.float32);d=len(x)
    def take(indices):
        indices=np.asarray(indices)
        return Jet(x[indices],np.eye(d)[indices],np.zeros(indices.shape+(d,d)))
    if case==0:
        a=take(np.arange(3));return (a*a).sum(0)
    if case==1:
        a=take([1,2]);return (a*a).sum(0)+take([2,3]).sum(0)
    if case==2:return (take(np.arange(6).reshape(2,3))+take([6,7,8]))*take([[9],[10]])
    if case==3:return take(np.arange(4).reshape(2,2)).matmul(take(np.arange(4,8).reshape(2,2)))
    if case==4:
        a=take([[2,3],[5,6]]).take([1,1,0],1);return (a*a).sum(1)
    if case==5:
        # Noncontiguous source is re-viewed in contiguous owner storage order.
        a=take(np.arange(1,7).reshape(3,2)).reshape((6,)).take([5,1,5,0],0);return a*a
    if case==6:
        a=take([0,1,2]).relu();return (a*a).sum(0)
    if case==7:return take([0,1,2]).norm(.25)
    if case==8:
        a=(take(np.arange(6).reshape(2,3)).norm(.125)*take([6,7,8])).relu();return (a*a).sum(1).sum(0)
    if case==9:
        a=take(np.arange(4).reshape(2,2));return a.matmul(a)
    assert case==10
    return take([0])

def wide(case,x):
    x=np.asarray(x,np.float64)
    def norm(a,eps):
        c=a-a.mean(axis=-1,keepdims=True)
        return c/np.sqrt((c*c).mean(axis=-1,keepdims=True)+eps)
    if case==0:return np.array([np.dot(x,x)])
    if case==1:return np.array([x[1]**2+x[2]**2+x[2]+x[3]])
    if case==2:return ((x[:6].reshape(2,3)+x[6:9])*x[9:11,None]).ravel()
    if case==3:return (x[:4].reshape(2,2)@x[4:8].reshape(2,2)).ravel()
    if case==4:return np.array([2*x[3]**2+x[2]**2,2*x[6]**2+x[5]**2])
    if case==5:return x[[6,2,6,1]]**2
    if case==6:return np.array([np.dot(np.maximum(x,0),np.maximum(x,0))])
    if case==7:return norm(x,.25).ravel()
    if case==8:return np.array([np.square(np.maximum(norm(x[:6].reshape(2,3),.125)*x[6:9],0)).sum()])
    if case==9:return (x.reshape(2,2)@x.reshape(2,2)).ravel()
    assert case==10
    return x.copy()

def finite_difference(case,x):
    d=len(x);y=wide(case,x);j=np.zeros((len(y),d));h=np.zeros((len(y),d,d))
    for a in range(d):
        delta=np.zeros(d);delta[a]=2**-15
        j[:,a]=(wide(case,x+delta)-wide(case,x-delta))/(2*delta[a])
        delta[a]=2**-9
        h[:,a,a]=(wide(case,x+delta)-2*y+wide(case,x-delta))/(delta[a]**2)
        for b in range(a):
            other=np.zeros(d);other[b]=2**-9
            h[:,a,b]=h[:,b,a]=(wide(case,x+delta+other)-wide(case,x+delta-other)-wide(case,x-delta+other)+wide(case,x-delta-other))/(4*delta[a]*other[b])
    return j,h

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('results',type=Path);p.add_argument('--record',type=Path);a=p.parse_args()
    points=fd_points=values=derivatives=0;max_analytic=0.;max_fd_j=max_fd_h=0.
    for line in a.results.read_text().splitlines():
        item=json.loads(line)
        if item.get('vmap'):
            expected=np.array([[0,2,4,1],[0,4,4,1],[0,-2,4,1]],np.float64)
            np.testing.assert_array_equal(np.asarray(item['gradient']).reshape(3,4),expected)
            continue
        case=item['case'];x=np.asarray(item['x'],np.float64);expected=jets(case,x);n,d=item['codomain'],item['domain']
        y=np.asarray(item['y']);j=np.asarray(item['jacobian']).reshape(n,d);h=np.asarray(item['hessian']).reshape(n,d,d)
        np.testing.assert_array_equal(y,expected.v.ravel().astype(np.float64))
        for actual,ref in [(j,expected.j.reshape(n,d)),(h,expected.h.reshape(n,d,d))]:
            np.testing.assert_allclose(actual,ref,atol=ANALYTIC_ATOL,rtol=ANALYTIC_RTOL)
            max_analytic=max(max_analytic,float(np.max(np.abs(actual-ref),initial=0)))
        np.testing.assert_allclose(item['jvp'],j@np.asarray(item['direction']),atol=ANALYTIC_ATOL,rtol=ANALYTIC_RTOL)
        np.testing.assert_allclose(h,np.swapaxes(h,1,2),atol=ANALYTIC_ATOL,rtol=ANALYTIC_RTOL)
        points+=1;values+=n;derivatives+=n*d+n*d*d
        if case!=6:  # Explicit ReLU0 convention is analytic-only.
            fj,fh=finite_difference(case,x)
            np.testing.assert_allclose(j,fj,atol=FD_J_ATOL,rtol=FD_J_RTOL)
            np.testing.assert_allclose(h,fh,atol=FD_H_ATOL,rtol=FD_H_RTOL)
            max_fd_j=max(max_fd_j,float(np.max(np.abs(j-fj),initial=0)));max_fd_h=max(max_fd_h,float(np.max(np.abs(h-fh),initial=0)));fd_points+=1
    assert points==33 and fd_points==30
    record={'schema':'psionic-function-composition-reference-v1','passed':True,'small_graph_points':points,'finite_difference_points':fd_points,'primal_values_exact':values,'formal_derivative_values':derivatives,'analytic_tolerance':{'atol':ANALYTIC_ATOL,'rtol':ANALYTIC_RTOL},'FD_jacobian_tolerance':{'atol':FD_J_ATOL,'rtol':FD_J_RTOL},'FD_hessian_tolerance':{'atol':FD_H_ATOL,'rtol':FD_H_RTOL},'max_analytic_abs_error':max_analytic,'max_FD_jacobian_abs_error':max_fd_j,'max_FD_hessian_abs_error':max_fd_h,'results_sha256':hashlib.sha256(a.results.read_bytes()).hexdigest(),'oracle_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scope':'Independent closed-form NumPy jets, smooth F64 finite differences, exact F32 stored primals; ReLU0 convention is analytic-only. No finite differences in production.'}
    if a.record:a.record.write_text(json.dumps(record,indent=2,sort_keys=True)+'\n')
    print(json.dumps(record,sort_keys=True))

if __name__=='__main__':main()
