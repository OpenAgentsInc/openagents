"""Independent tooling: homogeneous matrix product-rule tangents and metric FD.
No Coax product math, quaternion recursion, SDK or third-party package is used.
"""
import math

def identity():return [[float(i==j) for j in range(4)] for i in range(4)]
def zero():return [[0.]*4 for _ in range(4)]
def multiply(a,b):return [[math.fsum(a[i][k]*b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]
def add(a,b):return [[a[i][j]+b[i][j] for j in range(4)] for i in range(4)]
def fixed(offset,quaternion):
    w,x,y,z=quaternion;norm=math.sqrt(math.fsum(v*v for v in quaternion));w,x,y,z=(v/norm for v in quaternion)
    result=identity();rotation=[[w*w+x*x-y*y-z*z,2*(x*y-w*z),2*(x*z+w*y)],
      [2*(x*y+w*z),w*w-x*x+y*y-z*z,2*(y*z-w*x)],
      [2*(x*z-w*y),2*(y*z+w*x),w*w-x*x-y*y+z*z]]
    for i in range(3):result[i][:3]=rotation[i];result[i][3]=offset[i]
    return result
def local_motion(node,q,joint):
    motion=identity();tangent=zero()
    if joint<0:return motion,tangent
    if joint<7:
        theta=q[joint];c,s=math.cos(theta),math.sin(theta)
        motion[0][:2]=[c,-s];motion[1][:2]=[s,c]
        tangent[0][:2]=[-s,-c];tangent[1][:2]=[c,-s]
    else:
        delta=q[joint]-node['reference_metres']
        for i in range(3):motion[i][3]=node['axis'][i]*delta;tangent[i][3]=node['axis'][i]
    return motion,tangent
def body_tangents(nodes,q):
    """T_world_body and nine analytic first derivatives for every body."""
    output=[]
    for i,node in enumerate(nodes):
        joint=node['joint'] if i<12 or node['joint']<0 else node['joint']+7
        motion,dmotion=local_motion(node,q,joint);attachment=fixed(node['offset_metres'],node['fixed_quaternion_wxyz_raw']);local=multiply(attachment,motion);dlocal=multiply(attachment,dmotion)
        parent,dp=(identity(),[zero() for _ in range(9)]) if node['parent']<0 else output[node['parent']]
        pose=multiply(parent,local);derivatives=[]
        for k in range(9):
            derivative=multiply(dp[k],local)
            if k==joint:derivative=add(derivative,multiply(parent,dlocal))
            derivatives.append(derivative)
        output.append((pose,derivatives))
    return output
def metric_gravity(nodes,model,q):
    poses=body_tangents(nodes,q);linear=[];angular=[];gravity=[0.]*9
    for (pose,partials),body in zip(poses,model['bodies']):
        local=fixed([0.,0.,0.],body['inertial_quaternion_wxyz']);world_inertial=multiply(pose,local);body_linear=[];body_angular=[]
        for j,derivative in enumerate(partials):
            velocity=[derivative[i][3]+math.fsum(derivative[i][k]*body['inertial_position'][k] for k in range(3)) for i in range(3)]
            # WORLD angular generator is dR/dq * R^T, represented by its
            # antisymmetric components. This is not an axis cross product.
            omega_matrix=[[math.fsum(derivative[i][k]*pose[n][k] for k in range(3)) for n in range(3)] for i in range(3)]
            omega=[.5*(omega_matrix[2][1]-omega_matrix[1][2]),.5*(omega_matrix[0][2]-omega_matrix[2][0]),.5*(omega_matrix[1][0]-omega_matrix[0][1])]
            principal_velocity=[math.fsum(world_inertial[k][axis]*omega[k] for k in range(3)) for axis in range(3)]
            body_linear.append(velocity);body_angular.append(principal_velocity);gravity[j]-=body['mass']*math.fsum(velocity[k]*model['gravity'][k] for k in range(3))
        linear.append(body_linear);angular.append(body_angular)
    metric=[]
    for i in range(9):
        for j in range(9):
            terms=[]
            for b,body in enumerate(model['bodies']):
                terms.append(body['mass']*math.fsum(x*y for x,y in zip(linear[b][i],linear[b][j]))+math.fsum(body['principal_inertia'][k]*angular[b][i][k]*angular[b][j][k] for k in range(3)))
            metric.append(math.fsum(terms)+(model['robot_joints'][i]['armature'] if i==j else 0.))
    return metric,gravity
def inverse_force(nodes,model,q,v,a,h):
    metric,gravity=metric_gravity(nodes,model,q);partials=[]
    for k in range(9):
        plus=list(q);minus=list(q);plus[k]+=h;minus[k]-=h
        pm,_=metric_gravity(nodes,model,plus);mm,_=metric_gravity(nodes,model,minus);partials.append([(x-y)/(2*h) for x,y in zip(pm,mm)])
    velocity_bias=[]
    for i in range(9):velocity_bias.append(.5*math.fsum((partials[k][i*9+j]+partials[j][i*9+k]-partials[i][j*9+k])*v[j]*v[k] for j in range(9) for k in range(9)))
    bias=[gravity[i]+velocity_bias[i] for i in range(9)]
    force=[math.fsum(metric[i*9+j]*a[j] for j in range(9))+bias[i] for i in range(9)]
    return {'force':force,'bias':bias,'M':metric,'gravity':gravity,'velocity_bias':velocity_bias}
