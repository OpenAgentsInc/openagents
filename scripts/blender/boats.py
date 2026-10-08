"""Original oars, sloop, and wreck pieces; meter units, bow toward -Y.

The thin hull has an open cockpit and separate gunwales, ribs, and seats.
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


def hull(p, end=None):
    stations=[(-3.4,.06,.55),(-2.7,.7,.38),(-1.5,1.05,.24),(0,1.2,.20),(1.5,1.1,.25),(2.7,.7,.4),(3.1,.4,.55)]
    if end=='bow':stations=stations[:4]
    if end=='stern':stations=stations[3:]
    points=[]
    for y,width,keel in stations:
        points.extend([(-width,y,1.1),(-width*.78,y,keel+.1),(0,y,keel),(width*.78,y,keel+.1),(width,y,1.1)])
    faces=[]
    for row in range(len(stations)-1):
        for col in range(4):
            a=row*5+col
            faces.append((a,a+5,a+6,a+1))
    mesh=bpy.data.meshes.new('Hull');mesh.from_pydata(points,[],faces);mesh.update()
    obj=bpy.data.objects.new('Hull',mesh);bpy.context.scene.collection.objects.link(obj)
    obj.data.materials.append(p['plank'])
    solid=obj.modifiers.new('Planking','SOLIDIFY');solid.thickness=.06
    for row in range(len(stations)-1):
        y,w,_=stations[row]; yn,wn,_=stations[row+1]
        for sign in [-1,1]:c.beam('Gunwale',(sign*w,y,1.1),(sign*wn,yn,1.1),.055,p['wood'])
    for y,width,keel in stations[1:-1]:
        kit.box('Seat',(width*1.8,.22,.09),(0,y,.9),p['wood'])


def oars(p):
    for side in [-1,1]:
        x=side*.3
        c.beam('Shaft',(x,-1.1,.07),(x,1.1,.07),.028,p['wood'])
        kit.box('Blade',(.22,.55,.04),(x,-1.05,.07),p['plank'],bevel=.04)


def sloop(p):
    hull(p)
    c.beam('Mast',(0,-.4,.3),(0,-.4,6.2),.07,p['wood'],8)
    c.beam('Boom',(0,-.4,2),(0,2.4,2),.05,p['wood'])
    for x,y in [(-1,0),(1,0),(0,-3)]:c.beam('Rigging',(0,-.4,5.8),(x,y,1.05),.012,p['rope'],3)
    mesh=bpy.data.meshes.new('Sail');mesh.from_pydata([(0,-.35,5.7),(0,-.35,2.1),(0,2.2,2.1)],[],[(0,1,2)])
    obj=bpy.data.objects.new('Sail',mesh);bpy.context.scene.collection.objects.link(obj)
    obj.data.materials.append(p['white']);p['white'].use_backface_culling=False
    kit.box('Rudder',(.08,.55,.9),(0,3.05,.6),p['wood'])


MODELS={'oars':(oars,3999),'sloop':(sloop,3999),'wreck_bow':(lambda p:hull(p,'bow'),3999),'wreck_stern':(lambda p:hull(p,'stern'),3999)}
if __name__=='__main__':c.run(MODELS,'boats')
