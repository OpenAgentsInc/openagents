"""Original small beach props, under 600 triangles each, in meter units.

Run Blender headless with -- OUTPUT_DIRECTORY [MODEL ...]. No kit inputs.
"""
import math
import os
import sys
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(__file__))
import coast_common as c
import kit


def driftwood(p):
    c.beam('Trunk',(-1.4,0,.25),(1.4,.2,.35),.22,p['wood'],8)
    c.beam('Branch',(.1,.1,.3),(.5,.8,.45),.09,p['wood'])


def fence(p):
    for i in range(7):
        kit.box('Slat',(.11,.07,1.1),(i*.3-.9,0,.55),p['plank'])
    for z in [.3,.8]:
        c.beam('Tie',(-1,0,z),(1,0,z),.018,p['rope'],4)


def grass(p, seaweed=False):
    for i in range(11):
        a=i*2.4
        height=.04+(i%4)*.015 if seaweed else .3+(i%4)*.12
        spread=.7 if seaweed else .4
        c.beam('Stem',(.12*math.cos(a),.12*math.sin(a),0),
               (spread*math.cos(a),spread*math.sin(a),height),.024,
               p['kelp' if seaweed else 'green'],3)


def shell(p):
    for i in range(9):
        a=(i-4)*.16
        c.beam('ShellRib',(0,0,.03),(.22*math.sin(a),.3*math.cos(a),.06),.018,p['shell'],4)


def rope(p):
    for i in range(3):
        kit.ring('Coil',.24+i*.035,.022,(0,0,.035),p['rope'],segs=12,minor_segs=3)


def net(p):
    for i in range(7):
        k=-.6+i*.2
        c.beam('Warp',(k,-.6,.02),(k,.6,.02),.009,p['rope'],3)
        c.beam('Weft',(-.6,k,.025),(.6,k,.025),.009,p['rope'],3)


def crate(p):
    kit.box('Crate',(.7,.7,.65),(0,0,.325),p['plank'])
    for x in [-.36,.36]:
        for z in [.1,.55]:
            kit.box('Batten',(.05,.75,.08),(x,0,z),p['wood'])


def barrel(p):
    kit.lathe('Staves',[(0,0),(.28,0),(.35,.3),(.34,.65),(.27,.9),(0,.9)],material=p['plank'],segs=10)
    for z,r in [(.15,.32),(.7,.32)]:
        kit.ring('Hoop',r,.025,(0,0,z),p['iron'],segs=10,minor_segs=3)


MODELS={
 'driftwood':(driftwood,599),'dune_fence':(fence,599),
 'beach_grass':(grass,599),'shell':(shell,599),
 'seaweed':(lambda p:grass(p,True),599),'rope':(rope,599),
 'net':(net,599),'crate':(crate,599),'barrel':(barrel,599),
}
if __name__=='__main__': c.run(MODELS,'beach')
