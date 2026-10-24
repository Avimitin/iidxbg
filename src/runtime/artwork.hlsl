// Original atlas supplies the native mask; our media supplies the colour.
// c0 is the game's additive colour, set by its original draw dispatcher.
sampler2D native_art : register(s0);
sampler2D media : register(s1);
float4 additive : register(c0);
float4 tile : register(c8); // atlas left/top, inverse width/height
float4 crop : register(c9); // media scale xy, offset zw
float4 mode : register(c10); // x: use native alpha; y: transparent outside crop
float4 main(float2 uv : TEXCOORD0, float4 colour : COLOR0) : COLOR0 {
    float2 local = (uv - tile.xy) * tile.zw;
    float2 mapped = local * crop.xy + crop.zw;
    float4 art = tex2D(media, mapped);
    float mask = lerp(1.0, tex2D(native_art, uv).a, mode.x);
    float inside = step(0.0, mapped.x) * step(mapped.x, 1.0)
                 * step(0.0, mapped.y) * step(mapped.y, 1.0);
    art.a *= mask * lerp(1.0, inside, mode.y);
    return art * colour + additive;
}
