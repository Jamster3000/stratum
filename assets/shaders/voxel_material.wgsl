// Voxel material shader - fragment only extension with PBR lighting
// Uses UV_0 for atlas UV coordinates (min_u, min_v in xy, max_u, max_v passed via uv_b)
// UV_1.xy contains the quad size for tiling

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    pbr_functions,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

@group(2) @binding(100) var atlas_texture: texture_2d<f32>;
@group(2) @binding(101) var atlas_sampler: sampler;
// Ambient shadow tint: rgb = tint colour, a = opacity (how strongly to apply in dark areas)
@group(2) @binding(102) var<uniform> ambient_tint: vec4<f32>;


@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // UV_0 contains: (min_u, min_v) at corners, interpolated across quad
    // UV_1 contains: (uv_range_u, uv_range_v) - the size of UV region in atlas
    // We use fract() on the interpolated position to tile the texture
    
    // The atlas UV bounds are encoded as:
    // uv.x interpolates from min_u to max_u across the quad
    // uv.y interpolates from min_v to max_v across the quad
    // uv_b.x = uv_range (max_u - min_u)
    // uv_b.y = quad_width for tiling
    
    // Get the base UV (already interpolated to correct atlas position)
    let base_uv = in.uv;
    
    // Get tiling info from uv_b
    let uv_range = in.uv_b.x;  // Size of one texture in atlas UV space
    let tile_scale = in.uv_b.y; // How many times to tile (quad width/height)
    
    // Calculate tiled UV within the atlas region
    // fract() gives us 0-1 for each tile, then we scale to atlas region size
    let tiled_local = fract(base_uv / uv_range * tile_scale);
    
    // Map back to atlas space
    let min_uv = floor(base_uv / uv_range) * uv_range;
    let atlas_uv = min_uv + tiled_local * uv_range;
    
    // Sample from atlas
    let tex_color = textureSample(atlas_texture, atlas_sampler, atlas_uv);
    
    // Get PBR input from standard material for proper lighting
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Override the base color with our texture
    // Use the PBR input base color as the tint (avoids relying on a vertex color field that may be absent)
    pbr_input.material.base_color = tex_color * pbr_input.material.base_color;

    // Apply alpha discard if needed
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    // Apply PBR lighting (includes sun, skylight, shadows, ambient)
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);

    // Luminance-based semi-transparent shadow tint: darker and slightly coloured
    let lum = dot(out.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    // Slightly higher threshold so very-low lit faces still show texture
    let shadow_threshold = 0.12;
    // Clamp to 0..1 so we don't extrapolate the mix and fully blackout the color
    let dark_amount = clamp((shadow_threshold - lum) / shadow_threshold, 0.0, 1.0);
    let blend = ambient_tint.a * dark_amount;
    // Prevent shadows from ever reducing color to pure black by clamping the shadow tint
    let min_shadow_color = vec3<f32>(0.12, 0.12, 0.12);
    let shadow_color = max(ambient_tint.rgb, min_shadow_color);
    let mixed_rgb = mix(out.color.rgb, out.color.rgb * shadow_color, blend);

    // Safety: ensure the base texture albedo always contributes a small floor so faces never go pure black
    // This preserves texture detail in deep shadow while keeping shading intact.
    // Use a much smaller floor so nights can still be very dark
    // Lowered further so textures remain isible but deep darkness is preserved
    let albedo_floor = tex_color.rgb * 0.02;
    let final_rgb = max(mixed_rgb, albedo_floor);

    out.color = vec4<f32>(final_rgb, out.color.a);
#endif

    return out;
}
