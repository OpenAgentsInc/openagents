"""Original stone lighthouse, keeper's cottage, and bronze fog bell.

Meter units, tower centered at its base; no external assets. Run Blender
headless with -- OUTPUT_DIRECTORY [MODEL ...].
"""
import math
import os
import sys
sys.dont_write_bytecode = True
sys.path.insert(0,os.path.dirname(__file__))
import coast_common as c
import kit


def tower(p):
    kit.cyl('Plinth',4,1,(0,0,.5),p['rock'],verts=20)
    kit.cyl('Tower',3.2,18,(0,0,10),p['white'],verts=20,r2=2.1)
    for z in [3,7,11,15]:
        kit.cyl('StoneCourse',3.2-(z-1)*1.1/18+.06,.2,(0,0,z),p['rock'],verts=20)
    kit.cyl('Gallery',3.1,.35,(0,0,19.2),p['rock'],verts=20)
    for i in range(20):
        a=i*math.tau/20
        c.beam('RailPost',(2.9*math.cos(a),2.9*math.sin(a),19.3),(2.9*math.cos(a),2.9*math.sin(a),20.3),.055,p['iron'],4)
    kit.ring('GalleryRail',2.9,.06,(0,0,20.3),p['iron'],segs=20,minor_segs=4)
    for i in range(8):
        a=i*math.tau/8
        c.beam('LanternFrame',(1.8*math.cos(a),1.8*math.sin(a),19.4),(1.8*math.cos(a),1.8*math.sin(a),22),.09,p['iron'])
    lens=kit.mat('Coast_Lens',(1,.79,.38),.25,emit=(1,.6,.15),strength=3)
    kit.ball('Lens',.8,(0,0,20.6),lens,segs=16,rings=8)
    kit.cyl('Roof',2.4,1.5,(0,0,22.6),p['roof'],verts=16,r2=0)
    kit.box('Door',(1.25,.13,2.4),(0,-3.14,2.2),p['wood'])
    for z in [6,10,14]:
        kit.box('Window',(.65,.15,1.1),(0,-(3.2-(z-1)*1.1/18),z),p['roof'])


def bell(p):
    kit.lathe('Bell',[(0,.65),(.16,.6),(.22,.3),(.4,.08),(.35,.02),(.18,.2),(.08,.5),(0,.52)],material=p['shell'],segs=16)
    c.beam('Clapper',(0,0,.55),(0,0,.02),.035,p['iron'])
    for x in [-.55,.55]:c.beam('Support',(x,0,0),(x,0,1.1),.08,p['wood'])
    c.beam('Hanger',(-.6,0,1.05),(.6,0,1.05),.08,p['wood'])
    c.beam('Chain',(0,0,.6),(0,0,1.05),.02,p['iron'],4)


MODELS={'lighthouse':(tower,10000),'keeper_cottage':(lambda p:c.house(p,5,6,2.7),1500),'fog_bell':(bell,500)}
if __name__=='__main__':c.run(MODELS,'lighthouse')
