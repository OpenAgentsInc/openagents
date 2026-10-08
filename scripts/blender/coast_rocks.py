"""Original coastal rock modules; broad facets, meter units, ground origin.

Run Blender headless with -- OUTPUT_DIRECTORY [MODEL ...]. No kit inputs.
"""
import math
import os
import sys
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(__file__))
import coast_common as c
import kit


def cliff(p, angle=0):
    for i in range(5):
        x, y = (i-2)*2.3, 0
        if angle and i > 2:
            x, y = 2.3, (i-2)*2.3
        c.stone('Cliff', (3.4, 4.0, 10+i%3), (x, y, 4.2), p['rock'], i)


def inlet(p):
    for i in range(7):
        angle = math.pi * i / 6
        c.stone('Inlet', (3.5, 3.5, 8), (5*math.cos(angle), 5*math.sin(angle), 3), p['rock'], i+20)


def arch(p, cave=False):
    for side in [-1, 1]:
        c.stone('ArchPier', (3.5, 5 if cave else 3, 7), (side*3.5, 0, 3), p['rock'], side+7)
    for i in range(5):
        angle = i*math.pi/4
        c.stone('ArchCrown', (3, 5 if cave else 3, 2.5),
                (3.5*math.cos(angle), 0, 5.8+1.8*math.sin(angle)), p['rock'], i+10)


def shelf(p):
    # Open rings leave the C1 terrain and water visible inside the hollows.
    for x,y,r in [(-8,5,3.3), (8,5,3.3), (0,-9,3.3)]:
        for i in range(12):
            a=i*math.tau/12
            c.stone('PoolRim', (1.3, 1, 0.55), (x+r*math.cos(a), y+r*math.sin(a), 0.15), p['rock'], i)


MODELS = {
 'cliff_straight': (cliff, 4000),
 'cliff_corner': (lambda p: cliff(p, 1), 4000),
 'cliff_inlet': (inlet, 4000),
 'sea_stack': (lambda p: c.stone('Stack',(5,5,14),(0,0,6),p['rock'],31,3),4000),
 'sea_arch': (arch,4000),
 'sea_cave': (lambda p: arch(p,True),4000),
 'boulder': (lambda p: c.stone('Boulder',(3,2.5,2),(0,0,.75),p['rock'],42,3),4000),
 'tide_pool_shelf': (shelf,4000),
 'reef_rock': (lambda p: c.stone('ReefRock',(3,4,1.5),(0,0,.5),p['rock'],54),4000),
}
if __name__ == '__main__':
    c.run(MODELS, 'rocks')
