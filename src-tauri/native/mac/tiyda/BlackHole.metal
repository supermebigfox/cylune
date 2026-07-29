// Core ray-tracing port of s0xDk/ghostty-blackhole (MIT; see THIRD_PARTY_NOTICES.md).
#include <metal_stdlib>
using namespace metal;

struct Params { float2 resolution; float time, size, brightness, speed; uint style; float2 center; float ingestProgress, ejectProgress; };
struct VertexOut { float4 position [[position]]; float2 uv; };
struct Preset {
    float diskTemp, diskIncl, diskRoll, diskInner, diskOuter, diskOpacity, dopplerMix;
    float diskBeam, diskGain, diskContrast, diskWind, diskSpeed, starGain, exposure;
};

Preset presetForStyle(uint style) {
    Preset p={8500,1.45,.15,3.0,9.0,.65,1.0,3.0,1.0,.9,5.0,3.6,0.0,1.0};
    switch(style) {
        case 1: p={4500,1.52,.10,2.2,7.0,.85,.35,2.0,1.4,.5,5.0,3.6,0.0,1.2}; break;
        case 2: p={15000,1.30,.15,3.0,14.0,.35,1.0,4.0,1.2,1.3,8.0,3.6,0.0,.8}; break;
        case 3: p={6500,.30,0.0,3.0,10.0,.50,.8,2.5,1.0,1.1,5.0,3.6,0.0,1.0}; break;
        case 4: p={3800,.55,-.30,2.2,6.0,.45,.9,3.5,1.6,.4,3.0,2.5,0.0,1.1}; break;
        case 5: p={18000,1.05,.55,3.0,16.0,.30,1.0,5.0,1.0,1.5,9.0,6.0,0.0,.75}; break;
        case 6: p={5500,1.50,.35,1.8,8.0,.90,.6,2.5,2.2,1.6,7.0,5.0,0.0,1.4}; break;
        case 7: p={8500,1.45,.15,3.0,9.0,0.0,1.0,3.0,0.0,.9,5.0,3.6,.6,1.0}; break;
        case 8: p={7000,1.45,.15,3.5,7.0,.40,.5,2.0,.5,.3,3.0,1.5,0.0,.7}; break;
        default: break;
    }
    return p;
}

vertex VertexOut blackHoleVertex(uint id [[vertex_id]]) {
    constexpr float2 positions[] = {float2(-1,-1), float2(1,-1), float2(-1,1), float2(1,1)};
    VertexOut out; out.position=float4(positions[id],0,1); out.uv=float2(positions[id].x*0.5+0.5, 0.5-positions[id].y*0.5); return out;
}

float2 mirrorUV(float2 u) { return 1.0 - abs(1.0 - fmod(u, 2.0)); }
float2 wallpaperUV(float2 u, texture2d<float> desktop, float2 resolution) {
    float screenAspect=resolution.x/resolution.y;
    float textureAspect=float(desktop.get_width())/float(desktop.get_height());
    if(textureAspect>screenAspect) u.x=0.5+(u.x-0.5)*(screenAspect/textureAspect);
    else u.y=0.5+(u.y-0.5)*(textureAspect/screenAspect);
    return clamp(u,0.0,1.0);
}
float2 rot(float2 p, float a) { float c=cos(a), s=sin(a); return float2(c*p.x-s*p.y,s*p.x+c*p.y); }
float hash21(float2 p) { p=fract(p*float2(234.34,435.345)); p+=dot(p,p+34.23); return fract(p.x*p.y); }
float noise(float2 p) { float2 i=floor(p), f=fract(p); f=f*f*(3.0-2.0*f); return mix(mix(hash21(i),hash21(i+float2(1,0)),f.x),mix(hash21(i+float2(0,1)),hash21(i+1),f.x),f.y); }
float inflowContour(float angle, float normalizedRadius, float t) {
    float logarithmicRadius=log(max(normalizedRadius,1.02));
    // Constant-phase points travel toward a smaller radius as time advances.
    // The logarithmic radius bends each strand into a black-hole spiral
    // instead of making a scalloped or pulsating circular perimeter.
    float primary=pow(.5+.5*cos(angle*3.0-logarithmicRadius*7.2-t*3.35),7.0);
    float secondary=pow(.5+.5*cos(angle*5.0-logarithmicRadius*11.0-t*4.60),10.0);
    float organic=noise(float2(angle*1.35-t*.31,logarithmicRadius*4.20+t*1.16));
    return clamp(primary*.78+secondary*.34+organic*.13,0.0,1.0);
}
float2 inwardAccretionFlow(float2 p, float plen, float rh, float t, float activityGain, float flowDirection) {
    float safeRadius=max(rh,0.0001), normalizedRadius=plen/safeRadius;
    float coreGuard=smoothstep(1.03,1.32,normalizedRadius);
    float2 radial=plen>.0001?p/plen:float2(1,0);
    float2 clockwiseTangent=float2(-radial.y,radial.x);
    float angle=atan2(p.y,p.x);
    float radialFade=1.0-smoothstep(3.45,5.0,normalizedRadius);
    float contour=inflowContour(angle,normalizedRadius,t);
    float fullSurfaceEnvelope=coreGuard*radialFade;
    // Sampling farther from the event horizon makes the sampled desktop
    // appear at a smaller output radius: a one-way inward pull. The noise
    // coordinates advance toward smaller radii while rotating in one
    // direction, so the flow spirals inward without a water-wave reversal.
    float stream=noise(float2(angle*1.65-t*.96,normalizedRadius*2.10+t*2.62));
    float filament=noise(float2(angle*4.60-t*1.52,normalizedRadius*3.70+t*3.48));
    float spiralInflow=contour;
    float flowGain=1.50*activityGain;
    float radialPull=safeRadius*fullSurfaceEnvelope*flowGain*(1.38+1.30*spiralInflow+.82*stream);
    float rotationalPull=safeRadius*fullSurfaceEnvelope*flowGain*(1.02+.92*spiralInflow+.52*filament);
    return (radial*radialPull+clockwiseTangent*rotationalPull)*flowDirection;
}
float3 blackbody(float T) { float t=clamp(T,1500.0f,40000.0f)/100.0f; float r=t<=66?1.0:clamp(1.292936*pow(t-60.0,-0.1332047),0.0,1.0); float g=t<=66?clamp(0.3900816*log(t)-0.6318414,0.0,1.0):clamp(1.1298909*pow(t-60.0,-0.0755148),0.0,1.0); float b=t>=66?1.0:(t<=19?0.0:clamp(0.5432068*log(t-10.0)-1.196254,0.0,1.0)); return float3(r,g,b); }
float3 diskTintForStyle(uint style, float heat) {
    float h=clamp(heat,0.0,1.0);
    if(style==1) return mix(float3(.46,.012,.002),float3(1.00,.25,.012),h);
    return mix(float3(.025,.012,.34),float3(.40,.025,.92),h);
}

fragment float4 blackHoleFragment(VertexOut in [[stage_in]], texture2d<float> desktop [[texture(0)]], constant Params &P [[buffer(0)]]) {
    constexpr sampler linearSampler(filter::linear, address::clamp_to_edge);
    Preset S=presetForStyle(P.style);
    float ingest=clamp(P.ingestProgress,0.0,1.0), eject=clamp(P.ejectProgress,0.0,1.0);
    float ingestPulse=sin(3.14159265*ingest), ejectPulse=sin(3.14159265*eject);
    float activityGain=1.0+1.70*ingestPulse+1.10*ejectPulse;
    float flowDirection=1.0-2.15*ejectPulse;
    float2 uv=in.uv, res=P.resolution; float aspect=res.x/res.y;
    float t=P.time*P.speed+ingest*5.50-eject*4.80;
    float rh=0.125*P.size; float2 center = clamp(P.center, float2(0.0), float2(1.0));
    float2 p=(uv-center)*float2(aspect,1); float plen=length(p); float window=exp(-pow(plen/(7.0*rh),2.0));
    float2 spacetimeFlow=inwardAccretionFlow(p,plen,rh,t,activityGain,flowDirection);
    float normalizedRadius=plen/max(rh,.0001);
    float warpedRadius=length(p+spacetimeFlow*.24)/max(rh,.0001);
    float mask=1.0-smoothstep(3.10,5.0,warpedRadius);
    if (mask<0.002) return float4(0);
    constexpr float B=2.5980762, Z0=14.0;
    float W=B/max(rh,0.0001); float2 pr=rot(float2(p.x,-p.y),S.diskRoll)*W; float b=length(pr);
    if (b>S.diskOuter+3.0) { float defl=(2.0/(W*W))/max(plen,.0001)*(13.0/window*window)*window; float2 s=mirrorUV(center+(p+spacetimeFlow-normalize(p)*defl)/float2(aspect,1)); return float4(desktop.sample(linearSampler,wallpaperUV(s,desktop,res)).rgb,mask); }
    float3 x=float3(pr,Z0), v=float3(0,0,-1), prev=x; float h2=dot(pr,pr); float3 n=float3(0,sin(S.diskIncl),cos(S.diskIncl)); float prevPlane=dot(x,n); float3 emission=0; float trans=1; bool captured=false;
    for(uint i=0;i<40;i++) { float r2=dot(x,x); if(r2<1){captured=true;break;} if(x.z < -Z0 && v.z<0) break; float r=sqrt(r2), dt=clamp(.16*r,.03,1.5); float3 a=-1.5*h2*x/(r2*r2*r); v+=a*.5*dt; x+=v*dt; r2=dot(x,x); r=sqrt(r2); a=-1.5*h2*x/(r2*r2*r); v+=a*.5*dt; float plane=dot(x,n); if(plane*prevPlane<0 && trans>.02) { float f=prevPlane/(prevPlane-plane); float3 hit=mix(prev,x,f); float rc=length(hit); if(rc>S.diskInner && rc<S.diskOuter) { float phi=atan2(dot(hit,float3(0,cos(S.diskIncl),-sin(S.diskIncl))),hit.x); float grain=noise(float2(rc*2.8+phi*S.diskWind*.12,phi*3.0-t*S.diskSpeed*.55)); float contrastMix=clamp(S.diskContrast*.5,0.0,1.0); float streak=mix(1.0,.25+1.9*pow(grain,1.0+S.diskContrast),contrastMix); float band=smoothstep(S.diskInner,S.diskInner+.45,rc)*(1.0-smoothstep(max(S.diskInner+.5,S.diskOuter-2.4),S.diskOuter,rc)); float beta=clamp(rsqrt(max(2.0*(rc-1.0),.2)),0.0,.99); float gPhysics=sqrt(max(1.0-1.5/rc,.02))/max(1.0+beta*dot(normalize(cross(n,hit)),normalize(v)),.05); float g=mix(1.0,gPhysics,S.dopplerMix); float temp=pow(S.diskInner/rc,.75)*pow(max(1.0-sqrt(S.diskInner/rc),0.0),.25)/.488; float diskLuminousFlow=.24+2.82*pow(.5+.5*cos(phi*2.0-t*S.diskSpeed*1.82+rc*.92),10.0); float density=band*streak; emission+=trans*blackbody(S.diskTemp*temp*g)*(4.8*S.diskGain*density*diskLuminousFlow*temp*temp*pow(g,S.diskBeam)); trans*=1.0-clamp(S.diskOpacity*density,0.0,.95); } } prevPlane=plane; prev=x; }
    float2 flowingUV=center+(p+spacetimeFlow)/float2(aspect,1);
    float3 bg=desktop.sample(linearSampler,wallpaperUV(flowingUV,desktop,res)).rgb;
    bool shadow=captured && plen<rh*1.06;
    float2 starUV=uv;
    if(!shadow && !captured) { float3 d=normalize(v); if(d.z<-.05) { float q=(-13.0-x.z)/d.z; float2 sky=rot((x+d*q).xy,-S.diskRoll)/W; float2 suv=mirrorUV(center+(p+spacetimeFlow+(float2(sky.x,-sky.y)-p)*window)/float2(aspect,1)); starUV=suv; bg=desktop.sample(linearSampler,wallpaperUV(suv,desktop,res)).rgb; } }
    if(S.starGain>0.0) { float star=pow(hash21(floor(starUV*res/5.0)),32.0)*S.starGain; bg+=float3(.55,.72,1.0)*star; }
    float diskAbsorption=clamp((1.0-trans)*.22,0.0,.22);
    float3 physicalDisk=(1.0-exp(-emission*1.4*S.exposure))*P.brightness;
    float diskPeak=max(physicalDisk.r,max(physicalDisk.g,physicalDisk.b));
    float heat=clamp(length(emission)*.055,0.0,1.0);
    float luminousBreath=.82+.18*sin(t*2.05);
    float3 diskLight=diskTintForStyle(P.style,heat)*diskPeak*luminousBreath*(1.0+.45*ingestPulse);
    float diskOcclusion=clamp(diskAbsorption+diskPeak*.82,0.0,.92);
    float3 lit=bg*(1.0-diskOcclusion)+diskLight;
    float shadowEdge=shadow ? 1.0-smoothstep(rh*0.90,rh*1.06,plen) : 0.0;
    return float4(mix(lit,float3(0),shadowEdge),mask);
}
