"""Original reef plants and small rocks; meter units, rooted at z=0.

Each kelp strand stays below 200 triangles for later vertex animation.
Run Blender headless with -- OUTPUT_DIRECTORY [MODEL ...]. No kit inputs.
"""
import math
import os
import sys
sys.dont_write_bytecode = True
sys.path.insert(0,os.path.dirname(__file__))
import bpy
import coast_common as c
import kit


def kelp(p):
    points=[]
    for i in range(13):
        z=i*.22;x=.14*math.sin(i*.7);y=.07*math.cos(i*.4)
        width=.045+.09*math.sin(math.pi*i/13)
        points.extend([(x-width,y,z),(x+width,y,z)])
    faces=[(2*i,2*i+1,2*i+3,2*i+2) for i in range(12)]
    mesh=bpy.data.meshes.new('Kelp');mesh.from_pydata(points,[],faces);mesh.update()
    obj=bpy.data.objects.new('Kelp',mesh);bpy.context.scene.collection.objects.link(obj)
    obj.data.materials.append(p['kelp']);p['kelp'].use_backface_culling=False


def anemone(p):
    kit.cyl('Foot',.15,.1,(0,0,.05),p['shell'],verts=8)
    for i in range(12):
        a=i*math.tau/12
        c.beam('Tentacle',(.09*math.cos(a),.09*math.sin(a),.07),(.23*math.cos(a),.23*math.sin(a),.33),.018,p['red'],4)


def rocks(p):
    for i in range(3):
        c.stone('Reef',(.8,.6,.5),(i*.4-.4,.1*(i%2),.17),p['rock'],i,1)


MODELS={'kelp':(kelp,199),'anemone':(anemone,300),'reef_cluster':(rocks,300)}
if __name__=='__main__':c.run(MODELS,'reef')
