/** Original per-light math remains the default until the candidate shows a gain. */
export const WGSL_ORIGINAL_BRDF = `
fn brdf(n:vec3<f32>,v:vec3<f32>,l:vec3<f32>,base:vec3<f32>,rough:f32,metal:f32,dielectric:f32)->vec3<f32>{
 let nl=max(dot(n,l),0.0);let nv=max(dot(n,v),0.001);let h=safeNormalize(v+l,n);let nh=max(dot(n,h),0.0);let vh=max(dot(v,h),0.0);
 let a=rough*rough;let a2=a*a;let d=a2/(3.14159265*pow(nh*nh*(a2-1.0)+1.0,2.0)+0.00001);
 let vis=0.5/max(nl*sqrt(nv*nv*(1.0-a2)+a2)+nv*sqrt(nl*nl*(1.0-a2)+a2),0.00001);
 let f0=mix(vec3(dielectric),base,metal);let diffuseF=f0+(vec3(1.0)-f0)*pow(1.0-vh,5.0);let f=directFresnel(f0,vh);
 return ((vec3(1.0)-diffuseF)*base*(1.0-metal)/3.14159265+d*vis*f)*nl;
}
`;

/** Experimental reuse of light-independent GGX terms after material evaluation. */
export const WGSL_SURFACE_BRDF = `
struct BrdfTerms { nv:f32,a2:f32,viewRoot:f32,f0:vec3<f32> };
// b is base color, r is roughness, m is metalness, and d is dielectric reflectance.
fn brdfTerms(n:vec3<f32>,v:vec3<f32>,b:vec3<f32>,r:f32,m:f32,d:f32)->BrdfTerms{
 let nv=max(dot(n,v),0.001);let a=r*r;let a2=a*a;
 return BrdfTerms(nv,a2,sqrt(nv*nv*(1.0-a2)+a2),mix(vec3(d),b,m));
}
fn brdfLight(n:vec3<f32>,v:vec3<f32>,l:vec3<f32>,b:vec3<f32>,m:f32,s:BrdfTerms)->vec3<f32>{
 let nl=max(dot(n,l),0.0);let h=safeNormalize(v+l,n);let nh=max(dot(n,h),0.0);let vh=max(dot(v,h),0.0);
 let d=s.a2/(3.14159265*pow(nh*nh*(s.a2-1.0)+1.0,2.0)+0.00001);
 let vis=0.5/max(nl*s.viewRoot+s.nv*sqrt(nl*nl*(1.0-s.a2)+s.a2),0.00001);
 let diffuseF=s.f0+(vec3(1.0)-s.f0)*pow(1.0-vh,5.0);let f=directFresnel(s.f0,vh);
 return ((vec3(1.0)-diffuseF)*b*(1.0-m)/3.14159265+d*vis*f)*nl;
}
fn brdf(n:vec3<f32>,v:vec3<f32>,l:vec3<f32>,b:vec3<f32>,r:f32,m:f32,d:f32)->vec3<f32>{
 return brdfLight(n,v,l,b,m,brdfTerms(n,v,b,r,m,d));
}
`;

/** Rename only these owned identifiers; custom shader source never enters this step. */
export function compactSurfaceBrdf(source: string): string {
  return source
    .replace(/\bBrdfTerms\b/g, "BT")
    .replace(/\bbrdfTerms\b/g, "bt")
    .replace(/\bbrdfLight\b/g, "bl")
    .replace(/\bviewRoot\b/g, "vr")
    .replace(/\bdiffuseF\b/g, "df")
    .replace(/\btransmissionProfile\b/g, "tp")
    .replace(/\bnormalUV\b/g, "nu")
    .replace(/\bnormalScale\b/g, "ns")
    .replace(/\bsourceNormal\b/g, "sn")
    .replace(/\bskyFill\b/g, "sf");
}
