//! Live-renderer extensions to the first-frame painter's GLSL ports.
//! Shader math follows zui's gpui_wgpu (Apache-2.0).
use openagents_first_frame_paint::shaders as first;

pub(super) const EFFECTS: &str = r#"
uniform vec4 u_fade_y, u_fade_x, u_clip;
uniform float u_clip_radius;
float fade_alpha(vec2 p) {
    float r = 1.0;
    if (u_fade_y.z > 0.0) r = min(r, clamp((p.y-u_fade_y.x)/u_fade_y.z,0.0,1.0));
    if (u_fade_y.w > 0.0) r = min(r, clamp((u_fade_y.y-p.y)/u_fade_y.w,0.0,1.0));
    if (u_fade_x.z > 0.0) r = min(r, clamp((p.x-u_fade_x.x)/u_fade_x.z,0.0,1.0));
    if (u_fade_x.w > 0.0) r = min(r, clamp((u_fade_x.y-p.x)/u_fade_x.w,0.0,1.0));
    float a = r*r;
    if (u_clip.z > u_clip.x) {
        vec2 half_size = (u_clip.zw-u_clip.xy)*0.5;
        float radius = min(u_clip_radius,min(half_size.x,half_size.y));
        vec2 q = abs(p-u_clip.xy-half_size)-half_size+radius;
        float d = length(max(q,vec2(0.0)))+min(max(q.x,q.y),0.0)-radius;
        a *= clamp(0.5-d,0.0,1.0);
    }
    return a;
}
uniform vec4 u_bg0, u_bg1, u_bg_meta, u_bg_solid;
uniform vec2 u_stops;
vec3 srgb_to_linear(vec3 c) {
    return mix(c/12.92,pow((c+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),c));
}
vec3 linear_to_srgb(vec3 c) {
    return mix(c*12.92,1.055*pow(max(c,vec3(0.0)),vec3(1.0/2.4))-0.055,step(vec3(0.0031308),c));
}
vec3 to_oklab(vec3 c) {
    vec3 lms = mat3(0.4122214708,0.2119034982,0.0883024619,0.5363325363,0.6806995451,0.2817188376,0.0514459929,0.1073969566,0.6299787005)*c;
    lms = sign(lms)*pow(abs(lms),vec3(1.0/3.0));
    return mat3(0.2104542553,1.9779984951,0.0259040371,0.7936177850,-2.4285922050,0.7827717662,-0.0040720468,0.4505937099,-0.8086757660)*lms;
}
vec3 from_oklab(vec3 c) {
    vec3 lms = mat3(1.0,1.0,1.0,0.3963377774,-0.1055613458,-0.0894841775,0.2158037573,-0.0638541728,-1.2914855480)*c;
    lms = lms*lms*lms;
    return mat3(4.0767416621,-1.2684380046,-0.0041960863,-3.3077115913,2.6097574011,-0.7034186147,0.2309699292,-0.3413193965,1.7076147010)*lms;
}
vec4 background(vec2 p,vec4 b,vec4 solid) {
    if (u_bg_meta.x == 1.0) {
        float angle = (mod(u_bg_meta.y,360.0)-90.0)*3.14159265359/180.0;
        vec2 direction = vec2(cos(angle),sin(angle));
        if (b.z>b.w) direction.y *= b.w/b.z; else direction.x *= b.z/b.w;
        float t = dot(p-b.xy-b.zw*0.5,direction)/length(direction);
        t = abs(direction.x)>abs(direction.y) ? (t+b.z*0.5)/b.z : (t+b.w*0.5)/b.w;
        t = clamp((t-u_stops.x)/max(u_stops.y-u_stops.x,0.000001),0.0,1.0);
        vec3 a = u_bg_meta.z == 1.0 ? to_oklab(u_bg0.rgb) : linear_to_srgb(u_bg0.rgb);
        vec3 c = u_bg_meta.z == 1.0 ? to_oklab(u_bg1.rgb) : linear_to_srgb(u_bg1.rgb);
        vec3 rgb = mix(a,c,t);
        rgb = u_bg_meta.z == 1.0 ? from_oklab(rgb) : srgb_to_linear(rgb);
        return vec4(rgb,mix(u_bg0.a,u_bg1.a,t));
    }
    if (u_bg_meta.x >= 2.0) solid = u_bg_solid;
    if (u_bg_meta.x == 2.0) {
        float width = (u_bg_meta.y/65535.0)/255.0;
        float interval = mod(u_bg_meta.y,65535.0)/255.0;
        float period = (width+interval)*0.70710678;
        float x = mod(dot(p-b.xy,vec2(0.70710678,-0.70710678)),period);
        solid.a *= clamp(0.5-(min(x,period-x)-period*(width/(width+interval))*0.5),0.0,1.0);
    }
    if (u_bg_meta.x == 3.0) solid.a *= mod(floor((p.x-b.x)/u_bg_meta.y)+floor((p.y-b.y)/u_bg_meta.y),2.0);
    return solid;
}
"#;

// Keep the painter's SDF implementation shared. These hooks add live
// scene effects before and after that implementation.
pub(super) fn quad() -> String {
    let body = first::QUAD_FS
        .replace("void main()", "void paint_quad()")
        .replace(
            "vec4 bg = v_bg;",
            "vec4 bg = background(p, v_bounds, v_bg);",
        )
        .replace("over(bg, v_border)", "over(bg, dashed_border(p))");
    format!(
        r#"{EFFECTS}
uniform float u_dashed;
vec4 dashed_border(vec2 p);
{body}
vec4 dashed_border(vec2 p) {{
    vec4 c = v_border;
    if (u_dashed == 0.0) return c;
    vec2 size = v_bounds.zw;
    vec2 point = p-v_bounds.xy;
    vec2 center = point-size*0.5;
    float radius = pick_corner_radius(center,v_radii);
    vec2 corner = abs(center)-size*0.5+radius;
    bool unrounded = all(equal(v_radii,vec4(0.0)));
    bool horizontal = corner.x < corner.y;
    float t=0.0, max_t=0.0, velocity=0.0;
    if (unrounded) {{
        float width=horizontal?max(v_widths.x,v_widths.z):max(v_widths.y,v_widths.w);
        velocity=1.0/(3.0*max(width,0.000001));
        t=(horizontal?point.x:point.y)*velocity;
        max_t=(horizontal?size.x:size.y)*velocity;
    }} else {{
        vec4 dv=vec4(0.0);
        for(int i=0;i<4;i++) if(v_widths[i]>0.0) dv[i]=1.0/(3.0*v_widths[i]);
        float tr=v_radii.y,br=v_radii.z,bl=v_radii.w,tl=v_radii.x;
        float dt=dv.x,dr=dv.y,db=dv.z,dl=dv.w;
        float ctr=dt==0.0?dr:(dr==0.0?dt:min(dt,dr));
        float cbr=db==0.0?dr:(dr==0.0?db:min(db,dr));
        float cbl=db==0.0?dl:(dl==0.0?db:min(db,dl));
        float ctl=dt==0.0?dl:(dl==0.0?dt:min(dt,dl));
        float upto_tr=(size.x-tl-tr)*dt;
        float upto_r=upto_tr+tr*1.57079632679*ctr;
        float upto_br=upto_r+(size.y-tr-br)*dr;
        float upto_b=upto_br+br*1.57079632679*cbr;
        float upto_bl=upto_b+(size.x-br-bl)*db;
        float upto_l=upto_bl+bl*1.57079632679*cbl;
        float upto_tl=upto_l+(size.y-bl-tl)*dl;
        max_t=upto_tl+tl*1.57079632679*ctl;
        if(corner.x>=0.0 && corner.y>=0.0) {{
            float arc=atan(corner.y,corner.x)*radius;
            if(center.x>=0.0) {{
                if(center.y<0.0) {{velocity=ctr;t=upto_r-arc*velocity;}}
                else {{velocity=cbr;t=upto_br+arc*velocity;}}
            }} else {{
                if(center.y>=0.0) {{velocity=cbl;t=upto_l-arc*velocity;}}
                else {{velocity=ctl;t=upto_tl+arc*velocity;}}
            }}
        }} else if(horizontal) {{
            if(center.y<0.0) {{velocity=dt;t=(point.x-tl)*velocity;}}
            else {{velocity=db;t=upto_bl-(point.x-bl)*velocity;}}
        }} else {{
            if(center.x<0.0) {{velocity=dl;t=upto_tl-(point.y-tl)*velocity;}}
            else {{velocity=dr;t=upto_r+(point.y-tr)*velocity;}}
        }}
    }}
    float dash_length=2.0/3.0;
    if(unrounded) max_t-=dash_length;
    float period=0.0;
    if(max_t>=1.0) period=max_t/floor(max_t);
    else if(unrounded && max_t>dash_length) period=max_t;
    if(period>0.0 && velocity>0.0) {{
        float centered=mod(t+period*0.5-dash_length*0.5,period)-period*0.5;
        float distance=abs(centered)-dash_length*0.5;
        c.a*=clamp(0.5-distance/velocity,0.0,1.0);
    }}
    return c;
}}
void main() {{ paint_quad(); o.a *= fade_alpha(frag()); }}
"#
    )
}

pub(super) fn sprite() -> String {
    let body = first::SPRITE_FS
        .replace("void main()", "void paint_sprite()")
        .replace("float s = texture(u_tex, v_uv).r;", "float s = coverage();");
    format!(
        r#"{EFFECTS}
{body_head}
uniform vec2 u_atlas;
uniform float u_blur;
uniform vec4 u_tile;
float coverage() {{
    if(u_blur<=0.0) return texture(u_tex,v_uv).r;
    vec2 texel=1.0/u_atlas;
    vec2 lo=u_tile.xy/u_atlas, hi=(u_tile.xy+u_tile.zw)/u_atlas;
    vec2 middle=(lo+hi)*0.5;
    vec2 inset_lo=min(lo+0.5*texel,middle), inset_hi=max(hi-0.5*texel,middle);
    int radius=min(int(ceil(3.0*u_blur)),12);
    float accum=0.0,total=0.0;
    for(int y=-12;y<=12;y++) for(int x=-12;x<=12;x++) {{
        if(abs(x)>radius||abs(y)>radius) continue;
        vec2 offset=vec2(float(x),float(y));
        float weight=exp(-dot(offset,offset)/(2.0*u_blur*u_blur));
        vec2 uv=v_uv+offset*texel;
        float tap=0.0;
        if(all(greaterThanEqual(uv,lo))&&all(lessThanEqual(uv,hi))) tap=textureLod(u_tex,clamp(uv,inset_lo,inset_hi),0.0).r;
        accum+=weight*tap;total+=weight;
    }}
    return accum/max(total,0.000001);
}}
{body_main}
void main() {{ paint_sprite(); o.a *= fade_alpha(frag()); }}
"#,
        body_head = body.split("float color_brightness").next().unwrap(),
        body_main = format!(
            "float color_brightness{}",
            body.split_once("float color_brightness").unwrap().1
        )
    )
}

pub(super) fn image() -> String {
    let body = first::IMAGE_FS.replace("void main()", "void paint_image()");
    format!(
        r#"{EFFECTS}
uniform vec4 u_mask_bounds, u_mask_params;
uniform float u_bottom_feather;
{body}
void main() {{
    paint_image();
    float a = 1.0;
    if (u_mask_params.y>0.0) {{
        float d = quad_sdf(frag(),u_mask_bounds,vec4(u_mask_params.x));
        a = smoothstep(u_mask_params.z,u_mask_params.z+u_mask_params.y,d);
        if (u_bottom_feather>0.0) a = min(a,smoothstep(0.0,u_bottom_feather,u_mask_params.w-frag().y));
    }}
    o.a *= a*fade_alpha(frag());
}}
"#
    )
}

pub(super) const UNDERLINE_VS: &str = r#"
in vec4 a0; in vec4 a1; in vec4 a2; in vec4 a3;
flat out vec4 v_bounds; flat out vec4 v_mask; flat out vec4 v_color; flat out vec4 v_misc;
void main() { gl_Position=to_device(a0.xy+unit_vertex()*a0.zw); v_bounds=a0; v_mask=a1; v_color=a2; v_misc=a3; }
"#;
pub(super) const UNDERLINE_FS: &str = r#"
flat in vec4 v_bounds; flat in vec4 v_mask; flat in vec4 v_color; flat in vec4 v_misc;
void main() {
    vec2 p=frag(); if(clipped(p,v_mask)) discard;
    float a=v_color.a*v_color.a;
    if(v_misc.y!=0.0) {
        vec2 st=(p-v_bounds.xy)/v_bounds.w-vec2(0.0,0.5);
        float f=3.14159265359*2.0*v_misc.x/v_bounds.w;
        float amp=v_misc.x*0.8/v_bounds.w;
        float sine=sin(st.x*f)*amp;
        float ds=cos(st.x*f)*amp*f;
        float d=(st.y-sine)/sqrt(1.0+ds*ds)*v_bounds.w;
        a*=clamp(0.5-max(-d-v_misc.x*0.5,d-v_misc.x*0.5),0.0,1.0);
    }
    o=vec4(v_color.rgb,a);
}
"#;

pub(super) fn path() -> String {
    let body = first::PATH_FS.replace("void main()", "void paint_path()")
        .replace("o = vec4(v_color.rgb * v_color.a * alpha, v_color.a * alpha);",
            "vec4 c = background(frag(), u_path_bounds, v_color); o = vec4(c.rgb*c.a*alpha,c.a*alpha);");
    format!("{EFFECTS}\nuniform vec4 u_path_bounds;\n{body}\nvoid main() {{ paint_path(); }}")
}
