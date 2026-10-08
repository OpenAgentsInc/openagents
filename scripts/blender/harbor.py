"""Original harbor modules on a 2 m grid, fronts toward Blender -Y.

Run Blender headless with -- OUTPUT_DIRECTORY [MODEL ...]. No kit inputs.
"""
import os
import sys
sys.dont_write_bytecode = True
sys.path.insert(0,os.path.dirname(__file__))
import coast_common as c
import kit


def pier(p):
    for i in range(16):
        kit.box('DeckPlank',(3,.24,.16),(0,(i-7.5)*.25,0),p['plank'])
    for x in [-1.2,1.2]:
        for y in [-1.5,1.5]:
            kit.cyl('Piling',.18,4,(x,y,-1.5),p['wood'],verts=8)
        kit.box('Bearer',(.2,4,.25),(x,0,-.24),p['wood'])


def breakwater(p):
    for i in range(4):
        c.stone('Block',(2.2,3,2.4),((i-1.5)*1.8,0,.8),p['rock'],i)


def buoy(p):
    kit.cyl('Float',.5,.7,(0,0,0),p['red'],verts=10,r2=.24)
    kit.cyl('Mast',.065,1.6,(0,0,1),p['iron'],verts=6)
    kit.cyl('Mark',.27,.5,(0,0,1.6),p['red'],verts=6,r2=0)


def bollard(p):
    kit.cyl('Base',.3,.12,(0,0,.06),p['iron'],verts=8)
    kit.cyl('Stem',.12,.55,(0,0,.36),p['iron'],verts=8)
    c.beam('Crossbar',(-.32,0,.55),(.32,0,.55),.11,p['iron'],8)


def crane(p):
    kit.cyl('Base',.55,.3,(0,0,.15),p['rock'],verts=10)
    c.beam('Post',(0,0,.3),(0,0,3.6),.2,p['wood'],8)
    c.beam('Arm',(0,0,3.5),(2.8,0,3.5),.14,p['wood'])
    c.beam('Brace',(0,0,1.5),(2.2,0,3.5),.1,p['wood'])
    c.beam('Cable',(2.7,0,3.5),(2.7,0,1.1),.022,p['rope'],4)
    kit.ring('Hook',.15,.04,(2.7,0,1),p['iron'],segs=10,minor_segs=4,rot=(1.5708,0,0))


MODELS={
 'pier':(pier,2999),
 'piling':(lambda p:kit.cyl('Piling',.2,4,(0,0,2),p['wood'],verts=10),2999),
 'breakwater':(breakwater,2999),
 'boathouse':(lambda p:c.house(p,6,9,3.5),2999),
 'mooring':(lambda p:(bollard(p),kit.ring('Ring',.4,.03,(0,.5,.07),p['rope'],segs=12,minor_segs=3)),2999),
 'buoy':(buoy,2999),'bollard':(bollard,2999),'hand_crane':(crane,2999),
}
if __name__=='__main__':c.run(MODELS,'harbor')
