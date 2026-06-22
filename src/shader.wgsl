// Schwarzschild black-hole renderer, ported from
// opensource/ghostty-blackhole/blackhole.glsl
// (after Eric Bruneton's geodesic-traced black hole shader).
//
// Per-pixel null geodesic integration of the Binet acceleration
//   a = -(3/2) h² x / r⁵
// reproduces the exact Schwarzschild photon bending. Everything below is
// emergent from the integration:
//
//   * shadow            -- rays with b < B_CRIT spiral through the horizon
//   * gravitational lensing -- escaped rays project back onto the sky plane
//   * photon ring       -- rays winding near r = 1.5 r_s
//   * accretion disk    -- thin Keplerian disk pierced N times; Shakura-Sunyaev
//                          temperature + relativistic Doppler & beaming
//   * starfield         -- procedural sky lit by the *bent* ray direction
//
// Differences from the Ghostty original:
//   * iChannel0 (terminal contents) -> our captured desktop snapshot
//   * y axis: GLSL was top-down (Ghostty quirk); WGSL fragment coords are
//     top-down too, so the math carries over directly
//   * size + position come from the CPU animator (no token / pomodoro modes)
//   * work-area shield removed (no terminal prompt to protect)

struct Uniforms {
    resolution: vec2<f32>,
    time: f32,
    shadow_radius: f32,
    hole_center: vec2<f32>,
    intensity: f32,
    has_background: f32,
};

@group(0) @binding(0) var<uniform> U: Uniforms;
@group(0) @binding(1) var bg_tex: texture_2d<f32>;
@group(0) @binding(2) var bg_smp: sampler;

// -------------------------------------------------------------- tunables --
const LENS_DEPTH: f32 = 13.0;
const STAR_GAIN: f32 = 0.35;
const DISK_INNER: f32 = 1.8;
const DISK_OUTER: f32 = 8.0;
const DISK_INCL: f32 = 1.5;
const DISK_ROLL: f32 = 0.35;
const DISK_GAIN: f32 = 2.2;
const DISK_OPACITY: f32 = 0.9;
const DISK_TEMP: f32 = 5500.0;
const DOPPLER_MIX: f32 = 0.6;
const DISK_BEAM: f32 = 2.5;
const DISK_SPEED: f32 = 8.0;       // bumped from 5.0 -- more obvious orbit
const DISK_WIND: f32 = 7.0;
const DISK_CONTRAST: f32 = 2.0;    // bumped from 1.6 -- crisper filaments
const EXPOSURE: f32 = 1.4;
const DILATION_MIN: f32 = 0.55;    // floor higher so disk never visibly freezes

// critical impact parameter of a Schwarzschild hole, in r_s
const B_CRIT: f32 = 2.5980762;
const PI: f32 = 3.1415927;
const TAU: f32 = 6.2831853;

const N_STEPS: i32 = 48;

// ------------------------------------------------------------- vertex --
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vi & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, -y, 0.0, 1.0);
}

// ---------------------------------------------------------- helpers --
fn hash21(p_in: vec2<f32>) -> f32 {
    var p = fract(p_in * vec2<f32>(234.34, 435.345));
    p = p + dot(p, p + 34.23);
    return fract(p.x * p.y);
}

// value noise wrapping in y every perY cells (seamless across atan branch cut)
fn vnoise_wrap_y(p: vec2<f32>, per_y: f32) -> f32 {
    let i = floor(p);
    var f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    let y0 = (i.y - floor(i.y / per_y) * per_y);
    let y1 = ((i.y + 1.0) - floor((i.y + 1.0) / per_y) * per_y);
    let a = hash21(vec2<f32>(i.x, y0));
    let b = hash21(vec2<f32>(i.x + 1.0, y0));
    let c = hash21(vec2<f32>(i.x, y1));
    let d = hash21(vec2<f32>(i.x + 1.0, y1));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// WGSL fmod equivalent (positive remainder)
fn mod_pos(x: f32, y: f32) -> f32 {
    return x - y * floor(x / y);
}

fn mirror_uv(u: vec2<f32>) -> vec2<f32> {
    let m = vec2<f32>(mod_pos(u.x, 2.0), mod_pos(u.y, 2.0));
    return vec2<f32>(1.0) - abs(vec2<f32>(1.0) - m);
}

fn rot(v: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(c * v.x - s * v.y, s * v.x + c * v.y);
}

// Tanner Helland blackbody fit, normalized
fn blackbody(temp_k: f32) -> vec3<f32> {
    let t = clamp(temp_k, 1500.0, 40000.0) / 100.0;
    var r: f32;
    var g: f32;
    var b: f32;
    if (t <= 66.0) {
        r = 1.0;
    } else {
        r = clamp(1.292936 * pow(t - 60.0, -0.1332047), 0.0, 1.0);
    }
    if (t <= 66.0) {
        g = clamp(0.3900816 * log(t) - 0.6318414, 0.0, 1.0);
    } else {
        g = clamp(1.1298909 * pow(t - 60.0, -0.0755148), 0.0, 1.0);
    }
    if (t >= 66.0) {
        b = 1.0;
    } else if (t <= 19.0) {
        b = 0.0;
    } else {
        b = clamp(0.5432068 * log(t - 10.0) - 1.1962540, 0.0, 1.0);
    }
    return vec3<f32>(r, g, b);
}

// procedural starfield indexed by ray direction
fn stars(d: vec3<f32>) -> vec3<f32> {
    let sph = vec2<f32>(atan2(d.x, -d.z), asin(clamp(d.y, -1.0, 1.0)));
    let g = sph * 40.0;
    let id = floor(g);
    let h = hash21(id);
    if (h < 0.92) {
        return vec3<f32>(0.0);
    }
    let f = fract(g) - vec2<f32>(0.5);
    let off = (vec2<f32>(hash21(id + 17.3), hash21(id + 31.7)) - vec2<f32>(0.5)) * 0.7;
    let spark = smoothstep(0.10, 0.0, length(f - off));
    let tw = 0.7 + 0.3 * sin(U.time * (0.5 + 2.0 * hash21(id + 5.1)) + 40.0 * h);
    let tint = mix(vec3<f32>(1.0, 0.82, 0.60), vec3<f32>(0.75, 0.85, 1.0), hash21(id + 2.9));
    return tint * spark * tw * ((h - 0.92) / 0.08);
}

// Screen-space swirl ring just outside the shadow. Separate from the
// geodesic disk so it stays animated even when gravitational time
// dilation slows the real orbit, and visible in the analytic far-field
// branch where there is no disk integration.
fn halo_swirl(
    p: vec2<f32>,
    plen: f32,
    rh: f32,
    time: f32,
    intensity: f32,
) -> vec3<f32> {
    let ang = atan2(p.y, p.x);
    let rr = plen / max(rh, 1e-4);
    // Visible band 1.05..1.8 shadow radii out, Gaussian falloff.
    let ring = exp(-pow((rr - 1.30) / 0.35, 2.0));
    let phase = ang * 4.0 - time * 1.4 + rr * 2.5;
    let wisp = 0.55 + 0.45 * sin(phase) + 0.25 * sin(phase * 2.3 + 1.7);
    let color = mix(
        vec3<f32>(1.0, 0.55, 0.18),
        vec3<f32>(1.0, 0.85, 0.55),
        0.5 + 0.5 * sin(phase * 0.7),
    );
    return color * ring * max(wisp, 0.0) * 0.6 * (0.4 + 0.6 * intensity);
}

fn sample_bg(uv: vec2<f32>) -> vec3<f32> {
    if (U.has_background > 0.5) {
        return textureSample(bg_tex, bg_smp, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    }
    return vec3<f32>(0.0);
}

// ------------------------------------------------------------- fragment --
@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let res = U.resolution;
    let uv = frag.xy / res;
    let aspect = res.x / res.y;

    let t = U.time;
    let rh = max(U.shadow_radius, 1e-4);   // shadow radius in screen-height units
    let intensity = clamp(U.intensity, 0.0, 1.0);

    let rin = max(DISK_INNER, 1.6);
    let rout = max(DISK_OUTER, rin + 0.5);

    // disk pattern slows down (gravitational time dilation theme)
    let dil = mix(1.0, DILATION_MIN, intensity);

    // aspect-corrected frame centered on the hole (y top-down, like GLSL)
    let p = (uv - U.hole_center) * vec2<f32>(aspect, 1.0);
    let plen = length(p);

    // screen <-> world mapping: shadow's apparent radius = B_CRIT r_s == rh
    let W = B_CRIT / rh;
    let pr = rot(vec2<f32>(p.x, -p.y), DISK_ROLL) * W;
    let b = length(pr);

    // distance-window fade so lensing doesn't shimmer the whole screen
    let window = exp(-pow(plen / (7.0 * rh), 2.0));

    let bmax = rout + 3.0;
    let z0 = max(14.0, rout + 5.0);

    // ============ far field: analytic weak deflection ============
    if (b >= bmax) {
        let u = z0 * inverseSqrt(z0 * z0 + b * b);
        let defl = (2.0 / (W * W)) / max(plen, 1e-4)
                 * (1.29 * u + 0.07)
                 * max(LENS_DEPTH - 2.14 * u + 0.75, 0.0)
                 * window;
        let dir = p / max(plen, 1e-5);
        let ab = 0.035 * smoothstep(1.0, 2.0, b / bmax);
        var term = vec3<f32>(0.0);
        for (var i = 0; i < 3; i = i + 1) {
            let k = 1.0 + (f32(i) - 1.0) * ab;
            let sp = p - dir * defl * k;
            let suv = mirror_uv(U.hole_center + sp / vec2<f32>(aspect, 1.0));
            let s = sample_bg(suv);
            if (i == 0) { term.x = s.x; }
            else if (i == 1) { term.y = s.y; }
            else { term.z = s.z; }
        }
        let d = normalize(vec3<f32>(-(pr / b) * (2.0 / b), -1.0));
        let col = term + stars(d) * STAR_GAIN * window + halo_swirl(p, plen, rh, U.time, intensity);
        return vec4<f32>(col, 1.0);
    }

    // ============ near field: integrate the geodesic ============
    var x = vec3<f32>(pr, z0);
    var v = vec3<f32>(0.0, 0.0, -1.0);
    let h2 = dot(pr, pr);

    let ci = cos(DISK_INCL);
    let si = sin(DISK_INCL);
    let n = vec3<f32>(0.0, si, ci);
    let e2 = vec3<f32>(0.0, ci, -si);
    var sdir = 1.0;
    if (DISK_SPEED < 0.0) { sdir = -1.0; }
    let spd = abs(DISK_SPEED);

    var emitc = vec3<f32>(0.0);
    var trans = 1.0;
    var captured = false;
    var s_prev = dot(x, n);
    var x_prev = x;

    for (var i = 0; i < N_STEPS; i = i + 1) {
        var r2 = dot(x, x);
        if (r2 < 1.0) { captured = true; break; }
        if (x.z < -z0 && v.z < 0.0) { break; }
        if (r2 > 4.0 * z0 * z0) { break; }
        var r = sqrt(r2);
        let dt = clamp(0.16 * r, 0.03, 1.5);

        // leapfrog (kick-drift-kick)
        var a = -1.5 * h2 * x / (r2 * r2 * r);
        v = v + a * (0.5 * dt);
        x = x + v * dt;
        r2 = dot(x, x);
        r = sqrt(r2);
        a = -1.5 * h2 * x / (r2 * r2 * r);
        v = v + a * (0.5 * dt);

        let s = dot(x, n);
        if (s * s_prev < 0.0 && trans > 0.02) {
            let tc = s_prev / (s_prev - s);
            let xc = mix(x_prev, x, tc);
            let rc = length(xc);
            if (rc > rin && rc < rout) {
                let band = smoothstep(rin, rin * 1.25, rc)
                         * (1.0 - smoothstep(rout * 0.70, rout, rc));

                let phi = atan2(dot(xc, e2), xc.x);
                let turns = phi / TAU;
                let kep = pow(rin / rc, 1.5);
                let gloc = sqrt(max(1.0 - 1.5 / rc, 0.02));
                let swirl = rc * DISK_WIND * 0.12 - t * kep * spd * gloc * dil * sdir;

                let streaks_a = vnoise_wrap_y(vec2<f32>(rc * 2.8, turns * 19.0 + swirl * 3.0), 19.0) * 0.65;
                let streaks_b = vnoise_wrap_y(vec2<f32>(rc * 1.0, turns * 9.0 + swirl * 1.5 + 7.0), 9.0) * 0.35;
                var streaks = streaks_a + streaks_b;
                streaks = 0.35 + DISK_CONTRAST * streaks * streaks;

                let gasdir = normalize(cross(n, xc)) * sdir;
                let beta = clamp(inverseSqrt(max(2.0 * (rc - 1.0), 0.2)), 0.0, 0.99);
                var g = gloc / max(1.0 + beta * dot(gasdir, normalize(v)), 0.05);
                g = mix(1.0, g, DOPPLER_MIX);

                let xpr = max(1.0 - sqrt(rin / rc), 0.0);
                let tprof = pow(rin / rc, 0.75) * pow(xpr, 0.25) / 0.488;
                let cbb = blackbody(DISK_TEMP * tprof * g);
                let boost = pow(g, DISK_BEAM);

                let density = band * streaks;
                emitc = emitc + trans * cbb * (DISK_GAIN * 2.2 * density * tprof * tprof * boost);
                trans = trans * (1.0 - clamp(DISK_OPACITY * density, 0.0, 1.0));
            }
        }
        s_prev = s;
        x_prev = x;
    }
    if (!captured && dot(x, x) < 4.0) { captured = true; }

    // background contribution
    var bg = vec3<f32>(0.0);
    if (!captured) {
        let d = normalize(v);
        bg = bg + stars(d) * STAR_GAIN * window;
        if (d.z < -0.05) {
            let tpl = (-LENS_DEPTH - x.z) / d.z;
            let hp = x + d * tpl;
            let q = rot(vec2<f32>(hp.x, hp.y), -DISK_ROLL) / W;
            let sp = vec2<f32>(q.x, -q.y);
            let suv = mirror_uv(U.hole_center + (p + (sp - p) * window) / vec2<f32>(aspect, 1.0));
            let toward = smoothstep(0.05, 0.35, -d.z);
            bg = bg + sample_bg(suv) * toward;
        }
    }

    // HDR disk light tonemapped on top of the background, plus the swirl.
    let halo_glow = halo_swirl(p, plen, rh, U.time, intensity);
    let col = bg * trans + (vec3<f32>(1.0) - exp(-emitc * EXPOSURE)) + halo_glow;
    return vec4<f32>(col, 1.0);
}
