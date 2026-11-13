// Firm reason: 6 (lab-only Rust SDK adapter; Coax supplies scenes, metrics and orchestration).
use rapier3d_f64::prelude::*;
use std::{fs, time::Instant};
fn u(b: &[u8], at: usize) -> u64 {u64::from_le_bytes(b[at..at+8].try_into().unwrap())}
fn f(b: &[u8], at: usize) -> f64 {f64::from_bits(u(b,at))}
fn put(b: &mut [u8], at: usize, v: u64) {b[at..at+8].copy_from_slice(&v.to_le_bytes());}
fn snapshot(b: &mut [u8], at: usize, step: usize, world: &PhysicsWorld, ids: &[RigidBodyHandle]) {
    put(b,at,step as u64);
    for (i,id) in ids.iter().enumerate() {
        let body=&world.bodies[*id];let p=body.translation();let q=body.rotation();let v=body.linvel();let w=body.angvel();
        let values=[p.x,p.y,p.z,q.x,q.y,q.z,q.w,v.x,v.y,v.z,w.x,w.y,w.z];
        for (j,value) in values.iter().enumerate() {put(b,at+8+104*i+8*j,value.to_bits());}
    }
}
fn main() {
    let args: Vec<String>=std::env::args().collect();assert_eq!(args.len(),3);
    let b=fs::read(&args[1]).unwrap();assert_eq!(&b[..8],b"LABIN001");
    let count=u(&b,8) as usize;let steps=u(&b,16) as usize;let every=u(&b,24) as usize;
    assert!((1..=30000).contains(&count)&&steps==180&&every==30);assert_eq!(b.len(),160+count*104);
    let mut world=PhysicsWorld::new();world.gravity=Vector::new(f(&b,56),f(&b,64),f(&b,72));
    world.integration_parameters.dt=f(&b,48);world.integration_parameters.num_solver_iterations=u(&b,40) as usize;
    let normal=Vector::new(f(&b,80),f(&b,88),f(&b,96)).normalize();assert_eq!(f(&b,104),0.0);
    let mu=f(&b,144);let restitution=f(&b,152);
    world.insert(RigidBodyBuilder::fixed(),ColliderBuilder::new(SharedShape::new(HalfSpace::new(normal))).friction(mu).restitution(restitution));
    let mut ids=Vec::with_capacity(count);
    for i in 0..count {
        let at=160+104*i;let rotation=Rotation::from_xyzw(f(&b,at+24),f(&b,at+32),f(&b,at+40),f(&b,at+48));
        let pose=Pose::from_parts(Vector::new(f(&b,at),f(&b,at+8),f(&b,at+16)),rotation);
        let body=RigidBodyBuilder::dynamic().pose(pose).linvel(Vector::new(f(&b,at+56),f(&b,at+64),f(&b,at+72)))
            .angvel(Vector::new(f(&b,at+80),f(&b,at+88),f(&b,at+96))).can_sleep(false).ccd_enabled(false).linear_damping(0.0).angular_damping(0.0);
        let collider=ColliderBuilder::cuboid(f(&b,112),f(&b,120),f(&b,128)).mass(f(&b,136)).friction(mu).restitution(restitution);
        ids.push(world.insert(body,collider).0);
    }
    let mut out=vec![0u8;32+8*steps+(steps/every+1)*(8+104*count)];out[..8].copy_from_slice(b"LABOUT01");
    put(&mut out,8,count as u64);put(&mut out,16,steps as u64);put(&mut out,24,every as u64);
    let mut at=32+8*steps;snapshot(&mut out,at,0,&world,&ids);at+=8+104*count;
    for step in 1..=steps {let start=Instant::now();world.step();let elapsed=start.elapsed().as_nanos() as u64;put(&mut out,32+8*(step-1),elapsed);
        if step%every==0 {snapshot(&mut out,at,step,&world,&ids);at+=8+104*count;}}
    fs::write(&args[2],out).unwrap();
}
